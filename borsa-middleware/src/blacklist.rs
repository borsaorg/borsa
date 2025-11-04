//! Blacklisting middleware that temporarily gates connectors after rate-limit signals.
//!
//! Internal orchestrator calls flagged via [`CallOrigin::Internal`](borsa_core::CallOrigin)
//! bypass blacklist enforcement so compositional fan-outs do not poison the budget.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use borsa_core::connector::BorsaConnector;
use borsa_core::{BorsaError, CallContext, CallOrigin, Middleware};
#[cfg(feature = "tracing")]
use tracing::{debug, info, warn};

/// Middleware that blacklists its inner connector for a period upon quota exhaustion.
pub struct BlacklistConnector {
    inner: Arc<dyn BorsaConnector>,
    state: Mutex<Option<Instant>>, // blacklist-until; None means active
    default_duration: Duration,
}

impl BlacklistConnector {
    pub fn new(inner: Arc<dyn BorsaConnector>, default_duration: Duration) -> Self {
        let s = Self {
            inner,
            state: Mutex::new(None),
            default_duration,
        };
        #[cfg(feature = "tracing")]
        {
            info!(
                target = "borsa::middleware::blacklist",
                event = "init",
                default_duration_ms =
                    u64::try_from(default_duration.as_millis()).unwrap_or(u64::MAX),
                "initialized blacklist middleware"
            );
        }
        s
    }

    fn blacklist_remaining_ms(&self) -> Option<u64> {
        let mut guard = self.state.lock().expect("mutex poisoned");
        let now = Instant::now();
        if let Some(until) = *guard {
            if now < until {
                let remaining = until.saturating_duration_since(now);
                let ms: u64 = remaining.as_millis().try_into().unwrap_or(u64::MAX);
                return Some(ms.max(1));
            }
            // expired
            *guard = None;
        }
        None
    }

    fn blacklist_until(&self, until: Instant) {
        let mut guard = self.state.lock().expect("mutex poisoned");
        *guard = Some(until);
    }

    fn handle_error(&self, err: BorsaError) -> BorsaError {
        if let BorsaError::RateLimitExceeded {
            limit: _,
            window_ms,
        } = err.clone()
        {
            // Provider indicated an external rate limit. Honor the provider window when available
            // otherwise fall back to the configured default.
            let duration = if window_ms > 0 {
                Duration::from_millis(window_ms)
            } else {
                self.default_duration
            };
            self.blacklist_until(Instant::now() + duration);
            #[cfg(feature = "tracing")]
            warn!(
                target = "borsa::middleware::blacklist",
                event = "blacklist_set",
                window_ms = window_ms,
                applied_ms = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
                "blacklist window set due to upstream rate-limit"
            );
        }
        err
    }
}

/// Middleware config for constructing a [`BlacklistConnector`].
pub struct BlacklistMiddleware {
    pub duration: Duration,
}

impl BlacklistMiddleware {
    #[must_use]
    pub const fn new(duration: Duration) -> Self {
        Self { duration }
    }
}

impl Middleware for BlacklistMiddleware {
    fn apply(self: Box<Self>, inner: Arc<dyn BorsaConnector>) -> Arc<dyn BorsaConnector> {
        #[cfg(feature = "tracing")]
        {
            info!(
                target = "borsa::middleware::blacklist",
                event = "apply",
                default_duration_ms = u64::try_from(self.duration.as_millis()).unwrap_or(u64::MAX),
                "applying blacklist middleware"
            );
        }
        Arc::new(BlacklistConnector::new(inner, self.duration))
    }

    fn name(&self) -> &'static str {
        "BlacklistConnector"
    }

    fn config_json(&self) -> serde_json::Value {
        serde_json::json!({
            "default_duration_ms": self.duration.as_millis(),
        })
    }
}

#[borsa_macros::delegate_connector(inner)]
#[borsa_macros::delegate_all_providers(inner)]
impl BlacklistConnector {}

#[async_trait]
impl Middleware for BlacklistConnector {
    fn apply(self: Box<Self>, _inner: Arc<dyn BorsaConnector>) -> Arc<dyn BorsaConnector> {
        unreachable!("BlacklistConnector is already applied")
    }

    fn name(&self) -> &'static str {
        "BlacklistConnector"
    }

    fn config_json(&self) -> serde_json::Value {
        serde_json::json!({
            "default_duration_ms": self.default_duration.as_millis(),
        })
    }

    async fn pre_call(&self, ctx: &CallContext) -> Result<(), BorsaError> {
        if matches!(ctx.origin(), CallOrigin::Internal { .. }) {
            return Ok(());
        }
        #[cfg(feature = "tracing")]
        debug!(
            target = "borsa::middleware::blacklist",
            event = "pre_call",
            capability = %ctx.capability(),
            origin = ?ctx.origin(),
            "blacklist pre-call check"
        );
        if let Some(ms) = self.blacklist_remaining_ms() {
            #[cfg(feature = "tracing")]
            warn!(
                target = "borsa::middleware::blacklist",
                event = "blocked",
                capability = %ctx.capability(),
                reset_in_ms = ms,
                "blocked by blacklist"
            );
            return Err(BorsaError::TemporarilyBlacklisted { reset_in_ms: ms });
        }
        Ok(())
    }

    fn map_error(&self, err: BorsaError, ctx: &CallContext) -> BorsaError {
        if matches!(ctx.origin(), CallOrigin::Internal { .. }) {
            err
        } else {
            #[cfg(feature = "tracing")]
            debug!(
                target = "borsa::middleware::blacklist",
                event = "map_error",
                capability = %ctx.capability(),
                origin = ?ctx.origin(),
                %err,
                "blacklist mapping provider error"
            );
            self.handle_error(err)
        }
    }
}
