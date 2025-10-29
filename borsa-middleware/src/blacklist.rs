//! Blacklisting middleware that temporarily gates connectors after rate-limit signals.
//!
//! Internal orchestrator calls flagged via [`CallOrigin::Internal`](borsa_core::CallOrigin)
//! bypass blacklist enforcement so compositional fan-outs do not poison the budget.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use borsa_core::connector::BorsaConnector;
use borsa_core::{BorsaError, CallContext, CallOrigin, Middleware};

/// Middleware that blacklists its inner connector for a period upon quota exhaustion.
pub struct BlacklistingMiddleware {
    inner: Arc<dyn BorsaConnector>,
    state: Mutex<Option<Instant>>, // blacklist-until; None means active
    default_duration: Duration,
}

impl BlacklistingMiddleware {
    pub fn new(inner: Arc<dyn BorsaConnector>, default_duration: Duration) -> Self {
        Self {
            inner,
            state: Mutex::new(None),
            default_duration,
        }
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
        match err.clone() {
            BorsaError::QuotaExceeded {
                remaining,
                reset_in_ms,
            } => {
                // Only long-term exhaustion (remaining == 0) should trigger a longer blacklist.
                let duration = if remaining == 0 {
                    if reset_in_ms > 0 {
                        Duration::from_millis(reset_in_ms)
                    } else {
                        self.default_duration
                    }
                } else {
                    // Temporary slice exhaustion; still respect reset_in for brief blacklist to avoid immediate retries.
                    Duration::from_millis(reset_in_ms.max(1))
                };
                self.blacklist_until(Instant::now() + duration);
            }
            BorsaError::RateLimitExceeded {
                limit: _,
                window_ms,
            } => {
                // Provider indicated an external rate limit. Honor the provider window when available
                // otherwise fall back to the configured default.
                let duration = if window_ms > 0 {
                    Duration::from_millis(window_ms)
                } else {
                    self.default_duration
                };
                self.blacklist_until(Instant::now() + duration);
            }
            _ => {}
        }
        err
    }
}

/// Middleware config for constructing a [`BlacklistingMiddleware`].
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
        Arc::new(BlacklistingMiddleware::new(inner, self.duration))
    }

    fn name(&self) -> &'static str {
        "BlacklistingMiddleware"
    }

    fn config_json(&self) -> serde_json::Value {
        serde_json::json!({
            "default_duration_ms": self.duration.as_millis(),
        })
    }
}

#[borsa_macros::delegate_connector(inner)]
#[borsa_macros::delegate_all_providers(inner)]
impl BlacklistingMiddleware {}

#[async_trait]
impl Middleware for BlacklistingMiddleware {
    fn apply(self: Box<Self>, _inner: Arc<dyn BorsaConnector>) -> Arc<dyn BorsaConnector> {
        unreachable!("BlacklistingMiddleware is already applied")
    }

    fn name(&self) -> &'static str {
        "BlacklistingMiddleware"
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
        if let Some(ms) = self.blacklist_remaining_ms() {
            return Err(BorsaError::TemporarilyBlacklisted { reset_in_ms: ms });
        }
        Ok(())
    }

    fn map_error(&self, err: BorsaError, ctx: &CallContext) -> BorsaError {
        if matches!(ctx.origin(), CallOrigin::Internal { .. }) {
            err
        } else {
            self.handle_error(err)
        }
    }
}
