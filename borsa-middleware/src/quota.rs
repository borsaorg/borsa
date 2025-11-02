//! Quota-aware connector wrapper and implementations.
//!
//! Calls executed under [`CallOrigin::Internal`](borsa_core::CallOrigin) bypass quota
//! accounting so that orchestrator fan-outs do not consume end-user budget.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use borsa_core::connector::BorsaConnector;
use borsa_core::{BorsaError, CallContext, CallOrigin, Middleware};
use borsa_types::{QuotaConfig, QuotaConsumptionStrategy};

/// Wrapper that enforces quotas.
pub struct QuotaAwareConnector {
    inner: Arc<dyn BorsaConnector>,
    _config: QuotaConfig,
    runtime: Mutex<QuotaRuntime>,
}

struct QuotaRuntime {
    // Daily (window) tracking
    limit: u64,
    calls_made_in_window: u64,
    last_reset: Instant,
    window: Duration,

    // Hourly-spread tracking
    allowed_per_slice: u64,   // per-hour when strategy == EvenSpreadHourly
    slice_duration: Duration, // 1h slices
    calls_made_in_slice: u64,
    slice_start: Instant,
    strategy: QuotaConsumptionStrategy,
}

impl QuotaAwareConnector {
    /// Create a new quota-aware wrapper around an existing connector.
    pub fn new(inner: Arc<dyn BorsaConnector>, config: QuotaConfig) -> Self {
        let window = config.window;
        let limit = config.limit;
        // Compute hourly-spread slice parameters
        let strategy = config.strategy;
        let (allowed_per_slice, slice_duration) = match strategy {
            QuotaConsumptionStrategy::EvenSpreadHourly => {
                // Divide the configured window into 24 slices; for a 24h window this is 1h slices.
                let slices = 24u64;
                let per_slice = std::cmp::max(1, limit / slices);
                // Compute slice duration in milliseconds to handle small windows deterministically in tests.
                let window_ms = u128::from(u64::try_from(window.as_millis()).unwrap_or(u64::MAX));
                let raw_slice_ms = std::cmp::max(1u128, window_ms / u128::from(slices));
                let slice_ms = u64::try_from(raw_slice_ms).unwrap_or(u64::MAX);
                (per_slice, Duration::from_millis(slice_ms))
            }
            _ => (0, Duration::from_secs(0)),
        };

        Self {
            inner,
            _config: config,
            runtime: Mutex::new(QuotaRuntime {
                limit,
                calls_made_in_window: 0,
                last_reset: Instant::now(),
                window,

                allowed_per_slice,
                slice_duration,
                calls_made_in_slice: 0,
                slice_start: Instant::now(),
                strategy,
            }),
        }
    }

    /// Access the inner connector.
    pub fn inner(&self) -> &Arc<dyn BorsaConnector> {
        &self.inner
    }

    /// Check whether a call should be allowed under the configured quota strategy.
    ///
    /// # Errors
    /// Returns `BorsaError::QuotaExceeded` when the per-slice (for
    /// `EvenSpreadHourly`) or the overall window budget is exhausted. When the
    /// slice triggers the block but the daily window still has remaining
    /// units, `remaining` will be greater than zero and `reset_in_ms` reflects
    /// the time until the next slice boundary.
    ///
    /// # Panics
    /// Panics if the internal mutex is poisoned.
    pub fn should_allow_call(&self) -> Result<(), BorsaError> {
        let mut rt = self.runtime.lock().expect("mutex poisoned");
        let now = Instant::now();

        // Reset window if elapsed
        let elapsed = now.duration_since(rt.last_reset);
        if elapsed >= rt.window {
            rt.calls_made_in_window = 0;
            // Align last_reset to the beginning of the current window by calculating
            // how many complete windows have passed and advancing by that amount.
            // This ensures windows remain aligned to regular boundaries even with gaps in usage.
            let windows_passed = elapsed.as_nanos() / rt.window.as_nanos();
            let boundary_offset = Duration::from_nanos(
                (windows_passed * rt.window.as_nanos())
                    .try_into()
                    .unwrap_or(u64::MAX),
            );
            rt.last_reset += boundary_offset;
        }

        // Optional hourly-spread slice handling
        if matches!(rt.strategy, QuotaConsumptionStrategy::EvenSpreadHourly) {
            let elapsed = now.duration_since(rt.slice_start);
            if elapsed >= rt.slice_duration {
                rt.calls_made_in_slice = 0;
                // Align slice_start to the beginning of the current slice by calculating
                // how many complete slices have passed and advancing by that amount.
                // This ensures slices remain aligned to regular boundaries even with gaps in usage.
                let slices_passed = elapsed.as_nanos() / rt.slice_duration.as_nanos();
                let boundary_offset = Duration::from_nanos(
                    (slices_passed * rt.slice_duration.as_nanos())
                        .try_into()
                        .unwrap_or(u64::MAX),
                );
                rt.slice_start += boundary_offset;
            }

            // If slice is exhausted but daily window still has room, block temporarily
            if rt.calls_made_in_slice >= rt.allowed_per_slice && rt.calls_made_in_window < rt.limit
            {
                let elapsed_in_slice = now.duration_since(rt.slice_start);
                let reset_in_ms: u64 = rt
                    .slice_duration
                    .saturating_sub(elapsed_in_slice)
                    .as_millis()
                    .try_into()
                    .unwrap_or(u64::MAX);
                let remaining_units = rt.limit.saturating_sub(rt.calls_made_in_window);
                return Err(BorsaError::QuotaExceeded {
                    remaining: remaining_units,
                    reset_in_ms,
                });
            }
        }

        // Allow under overall window
        if rt.calls_made_in_window < rt.limit {
            rt.calls_made_in_window += 1;
            if matches!(rt.strategy, QuotaConsumptionStrategy::EvenSpreadHourly) {
                rt.calls_made_in_slice += 1;
            }
            return Ok(());
        }

        // Window exceeded
        let elapsed = now.duration_since(rt.last_reset);
        let remaining_ms = rt
            .window
            .saturating_sub(elapsed)
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX);
        let remaining_units = rt.limit.saturating_sub(rt.calls_made_in_window);
        let err = BorsaError::QuotaExceeded {
            remaining: remaining_units,
            reset_in_ms: remaining_ms,
        };
        drop(rt);
        Err(err)
    }

    fn translate_provider_error(err: BorsaError) -> BorsaError {
        match err {
            BorsaError::Connector { connector, error } => match *error {
                BorsaError::RateLimitExceeded { limit, window_ms } => {
                    BorsaError::RateLimitExceeded { limit, window_ms }
                }
                inner => BorsaError::Connector {
                    connector,
                    error: Box::new(inner),
                },
            },
            other => other,
        }
    }
}

/// Middleware config for constructing a [`QuotaAwareConnector`].
pub struct QuotaMiddleware {
    pub config: QuotaConfig,
}

impl QuotaMiddleware {
    #[must_use]
    pub const fn new(config: QuotaConfig) -> Self {
        Self { config }
    }
}

impl Middleware for QuotaMiddleware {
    fn apply(self: Box<Self>, inner: Arc<dyn BorsaConnector>) -> Arc<dyn BorsaConnector> {
        Arc::new(QuotaAwareConnector::new(inner, self.config))
    }

    fn name(&self) -> &'static str {
        "QuotaAwareConnector"
    }

    fn config_json(&self) -> serde_json::Value {
        let strategy = match self.config.strategy {
            QuotaConsumptionStrategy::EvenSpreadHourly => "EvenSpreadHourly",
            QuotaConsumptionStrategy::Weighted => "Weighted",
            _ => "Unit",
        };
        serde_json::json!({
            "limit": self.config.limit,
            "window_ms": self.config.window.as_millis(),
            "strategy": strategy,
        })
    }

    fn validate(&self, _ctx: &borsa_core::middleware::ValidationContext) -> Result<(), BorsaError> {
        // Optional: QuotaAware middleware works best with Blacklisting outermost (to handle quota errors)
        // but this is not strictly required. Validation is intentionally permissive to avoid breaking
        // existing usage patterns and allow flexible composition.
        Ok(())
    }
}

#[async_trait]
impl Middleware for QuotaAwareConnector {
    fn apply(self: Box<Self>, _inner: Arc<dyn BorsaConnector>) -> Arc<dyn BorsaConnector> {
        unreachable!("QuotaAwareConnector is already applied")
    }

    fn name(&self) -> &'static str {
        "QuotaAwareConnector"
    }

    fn config_json(&self) -> serde_json::Value {
        serde_json::json!({})
    }

    async fn pre_call(&self, ctx: &CallContext) -> Result<(), BorsaError> {
        if matches!(ctx.origin(), CallOrigin::Internal { .. }) {
            return Ok(());
        }
        self.should_allow_call()
    }

    fn map_error(&self, err: BorsaError, ctx: &CallContext) -> BorsaError {
        if matches!(ctx.origin(), CallOrigin::Internal { .. }) {
            err
        } else {
            Self::translate_provider_error(err)
        }
    }
}

#[borsa_macros::delegate_connector(inner)]
#[borsa_macros::delegate_all_providers(inner)]
impl QuotaAwareConnector {}
