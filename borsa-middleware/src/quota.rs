//! Quota-aware connector wrapper and implementations.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use borsa_core::connector::{
    AnalystPriceTargetProvider, BalanceSheetProvider, BorsaConnector, CalendarProvider,
    CashflowProvider, EarningsProvider, EsgProvider, HistoryProvider, IncomeStatementProvider,
    NewsProvider, OptionChainProvider, OptionsExpirationsProvider, ProfileProvider, QuoteProvider,
    RecommendationsProvider, RecommendationsSummaryProvider, SearchProvider,
    UpgradesDowngradesProvider,
};
use borsa_core::{AssetKind, BorsaError};
use borsa_types::{QuotaConfig, QuotaConsumptionStrategy, QuotaState};

/// Wrapper that enforces quotas.
pub struct QuotaAwareConnector {
    inner: Arc<dyn BorsaConnector>,
    _config: QuotaConfig,
    _state: Mutex<QuotaState>,
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
    pub fn new(inner: Arc<dyn BorsaConnector>, config: QuotaConfig, state: QuotaState) -> Self {
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
            _state: Mutex::new(state),
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
        if now.duration_since(rt.last_reset) >= rt.window {
            rt.calls_made_in_window = 0;
            rt.last_reset = now;
        }

        // Optional hourly-spread slice handling
        if matches!(rt.strategy, QuotaConsumptionStrategy::EvenSpreadHourly) {
            if now.duration_since(rt.slice_start) >= rt.slice_duration {
                rt.calls_made_in_slice = 0;
                rt.slice_start = now;
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
}

#[async_trait]
impl BorsaConnector for QuotaAwareConnector {
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    fn vendor(&self) -> &'static str {
        self.inner.vendor()
    }

    fn supports_kind(&self, kind: AssetKind) -> bool {
        self.inner.supports_kind(kind)
    }

    fn as_history_provider(&self) -> Option<&dyn HistoryProvider> {
        if self.inner.as_history_provider().is_some() {
            Some(self as &dyn HistoryProvider)
        } else {
            None
        }
    }
    fn as_quote_provider(&self) -> Option<&dyn QuoteProvider> {
        if self.inner.as_quote_provider().is_some() {
            Some(self as &dyn QuoteProvider)
        } else {
            None
        }
    }

    fn as_earnings_provider(&self) -> Option<&dyn EarningsProvider> {
        self.inner.as_earnings_provider()
    }
    fn as_income_statement_provider(&self) -> Option<&dyn IncomeStatementProvider> {
        self.inner.as_income_statement_provider()
    }
    fn as_balance_sheet_provider(&self) -> Option<&dyn BalanceSheetProvider> {
        self.inner.as_balance_sheet_provider()
    }
    fn as_cashflow_provider(&self) -> Option<&dyn CashflowProvider> {
        self.inner.as_cashflow_provider()
    }
    fn as_calendar_provider(&self) -> Option<&dyn CalendarProvider> {
        self.inner.as_calendar_provider()
    }
    fn as_recommendations_provider(&self) -> Option<&dyn RecommendationsProvider> {
        self.inner.as_recommendations_provider()
    }
    fn as_recommendations_summary_provider(&self) -> Option<&dyn RecommendationsSummaryProvider> {
        self.inner.as_recommendations_summary_provider()
    }
    fn as_upgrades_downgrades_provider(&self) -> Option<&dyn UpgradesDowngradesProvider> {
        self.inner.as_upgrades_downgrades_provider()
    }
    fn as_analyst_price_target_provider(&self) -> Option<&dyn AnalystPriceTargetProvider> {
        self.inner.as_analyst_price_target_provider()
    }
    fn as_major_holders_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::MajorHoldersProvider> {
        self.inner.as_major_holders_provider()
    }
    fn as_institutional_holders_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::InstitutionalHoldersProvider> {
        self.inner.as_institutional_holders_provider()
    }
    fn as_mutual_fund_holders_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::MutualFundHoldersProvider> {
        self.inner.as_mutual_fund_holders_provider()
    }
    fn as_insider_transactions_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::InsiderTransactionsProvider> {
        self.inner.as_insider_transactions_provider()
    }
    fn as_insider_roster_holders_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::InsiderRosterHoldersProvider> {
        self.inner.as_insider_roster_holders_provider()
    }
    fn as_net_share_purchase_activity_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::NetSharePurchaseActivityProvider> {
        self.inner.as_net_share_purchase_activity_provider()
    }
    fn as_profile_provider(&self) -> Option<&dyn ProfileProvider> {
        self.inner.as_profile_provider()
    }
    fn as_isin_provider(&self) -> Option<&dyn borsa_core::connector::IsinProvider> {
        self.inner.as_isin_provider()
    }
    fn as_search_provider(&self) -> Option<&dyn SearchProvider> {
        self.inner.as_search_provider()
    }
    fn as_esg_provider(&self) -> Option<&dyn EsgProvider> {
        self.inner.as_esg_provider()
    }
    fn as_news_provider(&self) -> Option<&dyn NewsProvider> {
        self.inner.as_news_provider()
    }
    fn as_options_expirations_provider(&self) -> Option<&dyn OptionsExpirationsProvider> {
        self.inner.as_options_expirations_provider()
    }
    fn as_option_chain_provider(&self) -> Option<&dyn OptionChainProvider> {
        self.inner.as_option_chain_provider()
    }
    fn as_stream_provider(&self) -> Option<&dyn borsa_core::connector::StreamProvider> {
        self.inner.as_stream_provider()
    }
}

#[async_trait]
impl HistoryProvider for QuotaAwareConnector {
    async fn history(
        &self,
        instrument: &borsa_core::Instrument,
        req: borsa_core::HistoryRequest,
    ) -> Result<borsa_core::HistoryResponse, BorsaError> {
        self.should_allow_call()?;
        let inner = self
            .inner
            .as_history_provider()
            .ok_or_else(|| BorsaError::unsupported("history"))?;
        match inner.history(instrument, req).await {
            Ok(response) => Ok(response),
            Err(BorsaError::Connector { connector, msg }) => {
                let lower = msg.to_lowercase();
                let looks_like_rate_limit = lower.contains("rate limit")
                    || lower.contains("429")
                    || lower.contains("too many requests");
                if looks_like_rate_limit {
                    // Heuristic: provider indicated a rate limit; expose as RateLimitExceeded.
                    return Err(BorsaError::RateLimitExceeded {
                        limit: 0,
                        window_ms: 0,
                    });
                }
                Err(BorsaError::Connector { connector, msg })
            }
            Err(other) => Err(other),
        }
    }

    fn supported_history_intervals(&self, kind: AssetKind) -> &'static [borsa_core::Interval] {
        if let Some(inner) = self.inner.as_history_provider() {
            inner.supported_history_intervals(kind)
        } else {
            &[]
        }
    }
}

#[async_trait]
impl QuoteProvider for QuotaAwareConnector {
    async fn quote(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<borsa_core::Quote, BorsaError> {
        self.should_allow_call()?;
        let inner = self
            .inner
            .as_quote_provider()
            .ok_or_else(|| BorsaError::unsupported("quote"))?;
        match inner.quote(instrument).await {
            Ok(response) => Ok(response),
            Err(BorsaError::Connector { connector, msg }) => {
                let lower = msg.to_lowercase();
                let looks_like_rate_limit = lower.contains("rate limit")
                    || lower.contains("429")
                    || lower.contains("too many requests");
                if looks_like_rate_limit {
                    return Err(BorsaError::RateLimitExceeded {
                        limit: 0,
                        window_ms: 0,
                    });
                }
                Err(BorsaError::Connector { connector, msg })
            }
            Err(other) => Err(other),
        }
    }
}
