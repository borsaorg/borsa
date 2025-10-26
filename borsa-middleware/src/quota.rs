//! Quota-aware connector wrapper and implementations.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use borsa_core::connector::{
    BorsaConnector, EsgProvider, HistoryProvider, NewsProvider, OptionChainProvider,
    OptionsExpirationsProvider, ProfileProvider, QuoteProvider, SearchProvider,
};
use borsa_core::{AssetKind, BorsaError, Middleware};
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

    fn translate_provider_error(err: BorsaError) -> BorsaError {
        match err {
            BorsaError::Connector { connector, msg } => {
                let lower = msg.to_lowercase();
                let looks_like_rate_limit = lower.contains("rate limit")
                    || lower.contains("429")
                    || lower.contains("too many requests");
                if looks_like_rate_limit {
                    BorsaError::RateLimitExceeded {
                        limit: 0,
                        window_ms: 0,
                    }
                } else {
                    BorsaError::Connector { connector, msg }
                }
            }
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
}

#[borsa_macros::delegate_connector(inner)]
impl QuotaAwareConnector {}

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
            Err(other) => Err(Self::translate_provider_error(other)),
        }
    }
}

#[async_trait]
impl borsa_core::connector::EarningsProvider for QuotaAwareConnector {
    async fn earnings(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<borsa_core::Earnings, BorsaError> {
        self.should_allow_call()?;
        let inner = self
            .inner
            .as_earnings_provider()
            .ok_or_else(|| BorsaError::unsupported("earnings"))?;
        match inner.earnings(instrument).await {
            Ok(r) => Ok(r),
            Err(e) => Err(Self::translate_provider_error(e)),
        }
    }
}

#[async_trait]
impl borsa_core::connector::IncomeStatementProvider for QuotaAwareConnector {
    async fn income_statement(
        &self,
        instrument: &borsa_core::Instrument,
        quarterly: bool,
    ) -> Result<Vec<borsa_core::IncomeStatementRow>, BorsaError> {
        self.should_allow_call()?;
        let inner = self
            .inner
            .as_income_statement_provider()
            .ok_or_else(|| BorsaError::unsupported("income_statement"))?;
        match inner.income_statement(instrument, quarterly).await {
            Ok(r) => Ok(r),
            Err(e) => Err(Self::translate_provider_error(e)),
        }
    }
}

#[async_trait]
impl borsa_core::connector::BalanceSheetProvider for QuotaAwareConnector {
    async fn balance_sheet(
        &self,
        instrument: &borsa_core::Instrument,
        quarterly: bool,
    ) -> Result<Vec<borsa_core::BalanceSheetRow>, BorsaError> {
        self.should_allow_call()?;
        let inner = self
            .inner
            .as_balance_sheet_provider()
            .ok_or_else(|| BorsaError::unsupported("balance_sheet"))?;
        match inner.balance_sheet(instrument, quarterly).await {
            Ok(r) => Ok(r),
            Err(e) => Err(Self::translate_provider_error(e)),
        }
    }
}

#[async_trait]
impl borsa_core::connector::CashflowProvider for QuotaAwareConnector {
    async fn cashflow(
        &self,
        instrument: &borsa_core::Instrument,
        quarterly: bool,
    ) -> Result<Vec<borsa_core::CashflowRow>, BorsaError> {
        self.should_allow_call()?;
        let inner = self
            .inner
            .as_cashflow_provider()
            .ok_or_else(|| BorsaError::unsupported("cashflow"))?;
        match inner.cashflow(instrument, quarterly).await {
            Ok(r) => Ok(r),
            Err(e) => Err(Self::translate_provider_error(e)),
        }
    }
}

#[async_trait]
impl borsa_core::connector::CalendarProvider for QuotaAwareConnector {
    async fn calendar(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<borsa_core::Calendar, BorsaError> {
        self.should_allow_call()?;
        let inner = self
            .inner
            .as_calendar_provider()
            .ok_or_else(|| BorsaError::unsupported("calendar"))?;
        match inner.calendar(instrument).await {
            Ok(r) => Ok(r),
            Err(e) => Err(Self::translate_provider_error(e)),
        }
    }
}

#[async_trait]
impl borsa_core::connector::RecommendationsProvider for QuotaAwareConnector {
    async fn recommendations(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<Vec<borsa_core::RecommendationRow>, BorsaError> {
        self.should_allow_call()?;
        let inner = self
            .inner
            .as_recommendations_provider()
            .ok_or_else(|| BorsaError::unsupported("recommendations"))?;
        match inner.recommendations(instrument).await {
            Ok(r) => Ok(r),
            Err(e) => Err(Self::translate_provider_error(e)),
        }
    }
}

#[async_trait]
impl borsa_core::connector::RecommendationsSummaryProvider for QuotaAwareConnector {
    async fn recommendations_summary(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<borsa_core::RecommendationSummary, BorsaError> {
        self.should_allow_call()?;
        let inner = self
            .inner
            .as_recommendations_summary_provider()
            .ok_or_else(|| BorsaError::unsupported("recommendations_summary"))?;
        match inner.recommendations_summary(instrument).await {
            Ok(r) => Ok(r),
            Err(e) => Err(Self::translate_provider_error(e)),
        }
    }
}

#[async_trait]
impl borsa_core::connector::UpgradesDowngradesProvider for QuotaAwareConnector {
    async fn upgrades_downgrades(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<Vec<borsa_core::UpgradeDowngradeRow>, BorsaError> {
        self.should_allow_call()?;
        let inner = self
            .inner
            .as_upgrades_downgrades_provider()
            .ok_or_else(|| BorsaError::unsupported("upgrades_downgrades"))?;
        match inner.upgrades_downgrades(instrument).await {
            Ok(r) => Ok(r),
            Err(e) => Err(Self::translate_provider_error(e)),
        }
    }
}

#[async_trait]
impl borsa_core::connector::AnalystPriceTargetProvider for QuotaAwareConnector {
    async fn analyst_price_target(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<borsa_core::PriceTarget, BorsaError> {
        self.should_allow_call()?;
        let inner = self
            .inner
            .as_analyst_price_target_provider()
            .ok_or_else(|| BorsaError::unsupported("analyst_price_target"))?;
        match inner.analyst_price_target(instrument).await {
            Ok(r) => Ok(r),
            Err(e) => Err(Self::translate_provider_error(e)),
        }
    }
}

#[async_trait]
impl borsa_core::connector::MajorHoldersProvider for QuotaAwareConnector {
    async fn major_holders(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<Vec<borsa_core::MajorHolder>, BorsaError> {
        self.should_allow_call()?;
        let inner = self
            .inner
            .as_major_holders_provider()
            .ok_or_else(|| BorsaError::unsupported("major_holders"))?;
        match inner.major_holders(instrument).await {
            Ok(r) => Ok(r),
            Err(e) => Err(Self::translate_provider_error(e)),
        }
    }
}

#[async_trait]
impl borsa_core::connector::InstitutionalHoldersProvider for QuotaAwareConnector {
    async fn institutional_holders(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<Vec<borsa_core::InstitutionalHolder>, BorsaError> {
        self.should_allow_call()?;
        let inner = self
            .inner
            .as_institutional_holders_provider()
            .ok_or_else(|| BorsaError::unsupported("institutional_holders"))?;
        match inner.institutional_holders(instrument).await {
            Ok(r) => Ok(r),
            Err(e) => Err(Self::translate_provider_error(e)),
        }
    }
}

#[async_trait]
impl borsa_core::connector::MutualFundHoldersProvider for QuotaAwareConnector {
    async fn mutual_fund_holders(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<Vec<borsa_core::InstitutionalHolder>, BorsaError> {
        self.should_allow_call()?;
        let inner = self
            .inner
            .as_mutual_fund_holders_provider()
            .ok_or_else(|| BorsaError::unsupported("mutual_fund_holders"))?;
        match inner.mutual_fund_holders(instrument).await {
            Ok(r) => Ok(r),
            Err(e) => Err(Self::translate_provider_error(e)),
        }
    }
}

#[async_trait]
impl borsa_core::connector::InsiderTransactionsProvider for QuotaAwareConnector {
    async fn insider_transactions(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<Vec<borsa_core::InsiderTransaction>, BorsaError> {
        self.should_allow_call()?;
        let inner = self
            .inner
            .as_insider_transactions_provider()
            .ok_or_else(|| BorsaError::unsupported("insider_transactions"))?;
        match inner.insider_transactions(instrument).await {
            Ok(r) => Ok(r),
            Err(e) => Err(Self::translate_provider_error(e)),
        }
    }
}

#[async_trait]
impl borsa_core::connector::InsiderRosterHoldersProvider for QuotaAwareConnector {
    async fn insider_roster_holders(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<Vec<borsa_core::InsiderRosterHolder>, BorsaError> {
        self.should_allow_call()?;
        let inner = self
            .inner
            .as_insider_roster_holders_provider()
            .ok_or_else(|| BorsaError::unsupported("insider_roster_holders"))?;
        match inner.insider_roster_holders(instrument).await {
            Ok(r) => Ok(r),
            Err(e) => Err(Self::translate_provider_error(e)),
        }
    }
}

#[async_trait]
impl borsa_core::connector::NetSharePurchaseActivityProvider for QuotaAwareConnector {
    async fn net_share_purchase_activity(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<Option<borsa_core::NetSharePurchaseActivity>, BorsaError> {
        self.should_allow_call()?;
        let inner = self
            .inner
            .as_net_share_purchase_activity_provider()
            .ok_or_else(|| BorsaError::unsupported("net_share_purchase_activity"))?;
        match inner.net_share_purchase_activity(instrument).await {
            Ok(r) => Ok(r),
            Err(e) => Err(Self::translate_provider_error(e)),
        }
    }
}

#[async_trait]
impl ProfileProvider for QuotaAwareConnector {
    async fn profile(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<borsa_core::Profile, BorsaError> {
        self.should_allow_call()?;
        let inner = self
            .inner
            .as_profile_provider()
            .ok_or_else(|| BorsaError::unsupported("profile"))?;
        match inner.profile(instrument).await {
            Ok(r) => Ok(r),
            Err(e) => Err(Self::translate_provider_error(e)),
        }
    }
}

#[async_trait]
impl borsa_core::connector::IsinProvider for QuotaAwareConnector {
    async fn isin(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<Option<borsa_core::Isin>, BorsaError> {
        self.should_allow_call()?;
        let inner = self
            .inner
            .as_isin_provider()
            .ok_or_else(|| BorsaError::unsupported("isin"))?;
        match inner.isin(instrument).await {
            Ok(r) => Ok(r),
            Err(e) => Err(Self::translate_provider_error(e)),
        }
    }
}

#[async_trait]
impl SearchProvider for QuotaAwareConnector {
    async fn search(
        &self,
        req: borsa_core::SearchRequest,
    ) -> Result<borsa_core::SearchResponse, BorsaError> {
        self.should_allow_call()?;
        let inner = self
            .inner
            .as_search_provider()
            .ok_or_else(|| BorsaError::unsupported("search"))?;
        match inner.search(req).await {
            Ok(r) => Ok(r),
            Err(e) => Err(Self::translate_provider_error(e)),
        }
    }
}

#[async_trait]
impl EsgProvider for QuotaAwareConnector {
    async fn sustainability(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<borsa_core::EsgScores, BorsaError> {
        self.should_allow_call()?;
        let inner = self
            .inner
            .as_esg_provider()
            .ok_or_else(|| BorsaError::unsupported("sustainability"))?;
        match inner.sustainability(instrument).await {
            Ok(r) => Ok(r),
            Err(e) => Err(Self::translate_provider_error(e)),
        }
    }
}

#[async_trait]
impl NewsProvider for QuotaAwareConnector {
    async fn news(
        &self,
        instrument: &borsa_core::Instrument,
        req: borsa_core::NewsRequest,
    ) -> Result<Vec<borsa_core::types::NewsArticle>, BorsaError> {
        self.should_allow_call()?;
        let inner = self
            .inner
            .as_news_provider()
            .ok_or_else(|| BorsaError::unsupported("news"))?;
        match inner.news(instrument, req).await {
            Ok(r) => Ok(r),
            Err(e) => Err(Self::translate_provider_error(e)),
        }
    }
}

#[async_trait]
impl OptionsExpirationsProvider for QuotaAwareConnector {
    async fn options_expirations(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<Vec<i64>, BorsaError> {
        self.should_allow_call()?;
        let inner = self
            .inner
            .as_options_expirations_provider()
            .ok_or_else(|| BorsaError::unsupported("options_expirations"))?;
        match inner.options_expirations(instrument).await {
            Ok(r) => Ok(r),
            Err(e) => Err(Self::translate_provider_error(e)),
        }
    }
}

#[async_trait]
impl OptionChainProvider for QuotaAwareConnector {
    async fn option_chain(
        &self,
        instrument: &borsa_core::Instrument,
        date: Option<i64>,
    ) -> Result<borsa_core::OptionChain, BorsaError> {
        self.should_allow_call()?;
        let inner = self
            .inner
            .as_option_chain_provider()
            .ok_or_else(|| BorsaError::unsupported("option_chain"))?;
        match inner.option_chain(instrument, date).await {
            Ok(r) => Ok(r),
            Err(e) => Err(Self::translate_provider_error(e)),
        }
    }
}

#[async_trait]
impl borsa_core::connector::StreamProvider for QuotaAwareConnector {
    async fn stream_quotes(
        &self,
        instruments: &[borsa_core::Instrument],
    ) -> Result<
        (
            borsa_core::stream::StreamHandle,
            tokio::sync::mpsc::Receiver<borsa_core::QuoteUpdate>,
        ),
        BorsaError,
    > {
        self.should_allow_call()?;
        let inner = self
            .inner
            .as_stream_provider()
            .ok_or_else(|| BorsaError::unsupported("stream_quotes"))?;
        match inner.stream_quotes(instruments).await {
            Ok(r) => Ok(r),
            Err(e) => Err(Self::translate_provider_error(e)),
        }
    }
}
