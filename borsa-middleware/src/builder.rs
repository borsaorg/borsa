use std::sync::Arc;
use std::time::Duration;

use borsa_core::Middleware;
use borsa_core::connector::BorsaConnector;
use borsa_types::{MiddlewareLayer, MiddlewareStack, QuotaConfig, QuotaConsumptionStrategy};
use serde_json::json;

/// Generic middleware builder for composing a connector with layered wrappers.
pub struct ConnectorBuilder {
    raw: Arc<dyn BorsaConnector>,
    layers: Vec<Box<dyn Middleware>>, // outermost first
}

impl ConnectorBuilder {
    /// Create a new builder from a raw, unwrapped connector.
    #[must_use]
    pub fn new(raw: Arc<dyn BorsaConnector>) -> Self {
        Self {
            raw,
            layers: Vec::new(),
        }
    }

    /// Internal: extract existing quota config from layers if present.
    fn existing_quota_config(&self) -> Option<QuotaConfig> {
        for layer in &self.layers {
            if layer.name() == "QuotaAwareConnector" {
                let cfg = layer.config_json();
                let defaults = QuotaConfig::default();
                let limit = cfg
                    .get("limit")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(defaults.limit);
                let window_ms = cfg
                    .get("window_ms")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or_else(|| {
                        u64::try_from(defaults.window.as_millis()).unwrap_or(u64::MAX)
                    });
                let strategy = match cfg.get("strategy").and_then(|v| v.as_str()) {
                    Some("EvenSpreadHourly") => QuotaConsumptionStrategy::EvenSpreadHourly,
                    Some("Weighted") => QuotaConsumptionStrategy::Weighted,
                    Some("Unit") => QuotaConsumptionStrategy::Unit,
                    _ => defaults.strategy,
                };
                return Some(QuotaConfig {
                    limit,
                    window: Duration::from_millis(window_ms),
                    strategy,
                });
            }
        }
        None
    }

    /// Add or replace quota configuration.
    #[must_use]
    pub fn with_quota(mut self, cfg: &QuotaConfig) -> Self {
        // Remove any existing Quota layer
        self.layers.retain(|m| m.name() != "QuotaAwareConnector");
        self.layers
            .insert(0, Box::new(crate::quota::QuotaMiddleware::new(cfg.clone())));
        self
    }

    /// Remove quota if present.
    #[must_use]
    pub fn without_quota(mut self) -> Self {
        self.layers.retain(|m| m.name() != "QuotaAwareConnector");
        self
    }

    /// Add or replace blacklist configuration.
    #[must_use]
    pub fn with_blacklist(mut self, duration: Duration) -> Self {
        self.layers.retain(|m| m.name() != "BlacklistingMiddleware");
        self.layers.insert(
            0,
            Box::new(crate::blacklist::BlacklistMiddleware::new(duration)),
        );
        self
    }

    /// Remove blacklist if present.
    #[must_use]
    pub fn without_blacklist(mut self) -> Self {
        self.layers.retain(|m| m.name() != "BlacklistingMiddleware");
        self
    }

    /// Shortcut: set quota limit only (preserves existing window/strategy if already set).
    #[must_use]
    pub fn quota_limit(self, limit: u64) -> Self {
        let mut cfg = self.existing_quota_config().unwrap_or_default();
        cfg.limit = limit;
        self.with_quota(&cfg)
    }

    /// Shortcut: set window (preserves existing limit/strategy if already set).
    #[must_use]
    pub fn quota_window(self, window: Duration) -> Self {
        let mut cfg = self.existing_quota_config().unwrap_or_default();
        cfg.window = window;
        self.with_quota(&cfg)
    }

    /// Shortcut: set strategy (preserves existing limit/window if already set).
    #[must_use]
    pub fn quota_strategy(self, strategy: QuotaConsumptionStrategy) -> Self {
        let mut cfg = self.existing_quota_config().unwrap_or_default();
        cfg.strategy = strategy;
        self.with_quota(&cfg)
    }

    /// Export the current middleware stack configuration for inspection.
    #[must_use]
    pub fn to_stack(&self) -> MiddlewareStack {
        let mut stack = MiddlewareStack::new();
        for layer in &self.layers {
            stack.push_inner(MiddlewareLayer::new(layer.name(), layer.config_json()));
        }
        // Document inner-most raw for observability only
        stack.push_inner(MiddlewareLayer::new(
            "RawConnector",
            json!({ "name": self.raw.name() }),
        ));
        stack
    }

    /// Construct a builder from a raw connector and an explicit stack.
    #[must_use]
    pub fn from_stack(raw: Arc<dyn BorsaConnector>, stack: &MiddlewareStack) -> Self {
        // Convert known layers to typed middleware; ignore unknowns for now.
        let mut layers: Vec<Box<dyn Middleware>> = Vec::new();
        for l in &stack.layers {
            match l.name.as_str() {
                "QuotaAwareConnector" => {
                    let limit = l
                        .config
                        .get("limit")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(1);
                    let window_ms = l
                        .config
                        .get("window_ms")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(60_000);
                    let strategy = match l.config.get("strategy").and_then(|v| v.as_str()) {
                        Some("EvenSpreadHourly") => QuotaConsumptionStrategy::EvenSpreadHourly,
                        Some("Weighted") => QuotaConsumptionStrategy::Weighted,
                        _ => QuotaConsumptionStrategy::Unit,
                    };
                    let cfg = QuotaConfig {
                        limit,
                        window: Duration::from_millis(window_ms),
                        strategy,
                    };
                    layers.push(Box::new(crate::quota::QuotaMiddleware::new(cfg)));
                }
                "BlacklistingMiddleware" => {
                    let dur_ms = l
                        .config
                        .get("default_duration_ms")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(300_000);
                    layers.push(Box::new(crate::blacklist::BlacklistMiddleware::new(
                        Duration::from_millis(dur_ms),
                    )));
                }
                _ => {}
            }
        }
        Self { raw, layers }
    }

    /// Build the wrapped connector according to the captured stack.
    #[must_use]
    pub fn build(self) -> Arc<dyn BorsaConnector> {
        // Apply outermost to innermost in order, threading through.
        let mut acc: Arc<dyn BorsaConnector> = Arc::clone(&self.raw);
        for m in self.layers.into_iter().rev() {
            acc = m.apply(acc);
        }
        acc
    }

    /// Add an arbitrary middleware layer (outermost by default).
    #[must_use]
    pub fn layer(mut self, layer: Box<dyn Middleware>) -> Self {
        self.layers.insert(0, layer);
        self
    }
}
