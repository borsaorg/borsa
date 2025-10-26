//! Builder for composing connectors with middleware layers.
//!
//! # Middleware Ordering Convention
//!
//! Middleware layers form an "onion" around the raw connector:
//!
//! ```text
//! User Request
//!     ↓
//! Outermost Middleware (e.g., Blacklist - checks first, handles errors last)
//!     ↓
//! Inner Middleware (e.g., Quota - enforces limits, translates errors)
//!     ↓
//! Raw Connector (e.g., YFinance - makes actual API calls)
//! ```
//!
//! ## Storage vs Application Order
//!
//! The `layers` vector stores middleware in **outermost-first** order for intuitive
//! builder semantics (last added = outermost), but they are **applied in reverse**
//! during `build()` to construct the proper nesting.
//!
//! Example:
//! ```text
//! builder.with_quota(..).with_blacklist(..)
//!
//! Storage: [Blacklist, Quota]  (outermost first)
//! Applied:  Raw -> Quota -> Blacklist  (innermost to outermost)
//! Result:   Blacklist(Quota(Raw))
//! ```
//!
//! This convention matches [`MiddlewareStack`](borsa_types::MiddlewareStack) where
//! `layers[0]` is the outermost layer.

use std::sync::Arc;
use std::time::Duration;

use borsa_core::Middleware;
use borsa_core::connector::BorsaConnector;
use borsa_types::{MiddlewareLayer, MiddlewareStack, QuotaConfig, QuotaConsumptionStrategy};
use serde_json::json;

/// Generic middleware builder for composing a connector with layered wrappers.
///
/// See [module-level documentation](self) for details on middleware ordering.
pub struct ConnectorBuilder {
    raw: Arc<dyn BorsaConnector>,
    /// Middleware layers in outermost-first order.
    ///
    /// During `build()`, these are applied in reverse (innermost to outermost)
    /// to construct the proper nesting: `layers[0](layers[1](...(raw)))`.
    layers: Vec<Box<dyn Middleware>>,
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
    ///
    /// Adds quota middleware at the outermost position (index 0) so it runs first
    /// on the request path and last on error handling. This ensures quota checks
    /// happen before the raw connector is called.
    ///
    /// If quota middleware already exists, it is removed and replaced.
    #[must_use]
    pub fn with_quota(mut self, cfg: &QuotaConfig) -> Self {
        self.layers.retain(|m| m.name() != "QuotaAwareConnector");
        // Insert at position 0 to make this the outermost layer
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
    ///
    /// Adds blacklist middleware at the outermost position (index 0) so it checks
    /// blacklist state before any other middleware runs, and handles quota/rate-limit
    /// errors to update blacklist state.
    ///
    /// If blacklist middleware already exists, it is removed and replaced.
    #[must_use]
    pub fn with_blacklist(mut self, duration: Duration) -> Self {
        self.layers.retain(|m| m.name() != "BlacklistingMiddleware");
        // Insert at position 0 to make this the outermost layer
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
    ///
    /// Returns a [`MiddlewareStack`] that preserves the outermost-first ordering
    /// convention. The resulting stack can be serialized, stored, and later
    /// reconstructed with [`from_stack`](Self::from_stack).
    ///
    /// The raw connector is appended as the innermost "layer" for observability.
    #[must_use]
    pub fn to_stack(&self) -> MiddlewareStack {
        let mut stack = MiddlewareStack::new();
        // Iterate in storage order (outermost first) and push_inner to maintain convention
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
    ///
    /// Reconstructs middleware layers from a serialized [`MiddlewareStack`],
    /// preserving the outermost-first ordering convention. Unknown middleware
    /// types are silently ignored (forward compatibility).
    ///
    /// This is the inverse of [`to_stack`](Self::to_stack).
    #[must_use]
    pub fn from_stack(raw: Arc<dyn BorsaConnector>, stack: &MiddlewareStack) -> Self {
        // Convert known layers to typed middleware; ignore unknowns for forward compatibility
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
    ///
    /// Applies middleware layers in reverse order (innermost to outermost) to construct
    /// the proper nesting. Since `layers` stores middleware in outermost-first order,
    /// we reverse during iteration to apply them innermost-first.
    ///
    /// Example with `layers = [Blacklist, Quota]`:
    /// ```text
    /// 1. Start:  acc = Raw
    /// 2. Apply Quota (last in vec):     acc = Quota(Raw)
    /// 3. Apply Blacklist (first in vec): acc = Blacklist(Quota(Raw))
    /// ```
    ///
    /// The resulting connector processes requests from outermost to innermost:
    /// `User -> Blacklist -> Quota -> Raw`
    #[must_use]
    pub fn build(self) -> Arc<dyn BorsaConnector> {
        let mut acc: Arc<dyn BorsaConnector> = Arc::clone(&self.raw);
        // Reverse iteration: apply innermost middleware first, outermost last
        for m in self.layers.into_iter().rev() {
            acc = m.apply(acc);
        }
        acc
    }

    /// Add an arbitrary middleware layer at the outermost position.
    ///
    /// This method inserts the layer at index 0, making it the first to receive
    /// requests and the last to handle errors. Use this for custom middleware that
    /// should wrap all other layers.
    #[must_use]
    pub fn layer(mut self, layer: Box<dyn Middleware>) -> Self {
        // Insert at position 0 to make this the outermost layer
        self.layers.insert(0, layer);
        self
    }
}
