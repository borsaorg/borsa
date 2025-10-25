use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use borsa_core::connector::{
    AnalystPriceTargetProvider, BalanceSheetProvider, BorsaConnector, CalendarProvider,
    CashflowProvider, EarningsProvider, EsgProvider, HistoryProvider, IncomeStatementProvider,
    MajorHoldersProvider, NewsProvider, OptionChainProvider, OptionsExpirationsProvider,
    ProfileProvider, QuoteProvider, RecommendationsProvider, RecommendationsSummaryProvider,
    SearchProvider, StreamProvider, UpgradesDowngradesProvider,
};
use borsa_core::{AssetKind, BorsaError};

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

    fn is_blacklisted(&self) -> bool {
        let mut guard = self.state.lock().expect("mutex poisoned");
        let now = Instant::now();
        if let Some(until) = *guard {
            if now < until {
                return true;
            }
            // expired
            *guard = None;
        }
        false
    }

    fn blacklist_until(&self, until: Instant) {
        let mut guard = self.state.lock().expect("mutex poisoned");
        *guard = Some(until);
    }

    fn handle_error(&self, err: BorsaError) -> BorsaError {
        if let BorsaError::QuotaExceeded {
            remaining,
            reset_in_ms,
        } = err.clone()
        {
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
        err
    }
}

#[async_trait]
impl BorsaConnector for BlacklistingMiddleware {
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
        if self.inner.as_earnings_provider().is_some() {
            Some(self as &dyn EarningsProvider)
        } else {
            None
        }
    }
    fn as_income_statement_provider(&self) -> Option<&dyn IncomeStatementProvider> {
        if self.inner.as_income_statement_provider().is_some() {
            Some(self as &dyn IncomeStatementProvider)
        } else {
            None
        }
    }
    fn as_balance_sheet_provider(&self) -> Option<&dyn BalanceSheetProvider> {
        if self.inner.as_balance_sheet_provider().is_some() {
            Some(self as &dyn BalanceSheetProvider)
        } else {
            None
        }
    }
    fn as_cashflow_provider(&self) -> Option<&dyn CashflowProvider> {
        if self.inner.as_cashflow_provider().is_some() {
            Some(self as &dyn CashflowProvider)
        } else {
            None
        }
    }
    fn as_calendar_provider(&self) -> Option<&dyn CalendarProvider> {
        if self.inner.as_calendar_provider().is_some() {
            Some(self as &dyn CalendarProvider)
        } else {
            None
        }
    }
    fn as_recommendations_provider(&self) -> Option<&dyn RecommendationsProvider> {
        if self.inner.as_recommendations_provider().is_some() {
            Some(self as &dyn RecommendationsProvider)
        } else {
            None
        }
    }
    fn as_recommendations_summary_provider(&self) -> Option<&dyn RecommendationsSummaryProvider> {
        if self.inner.as_recommendations_summary_provider().is_some() {
            Some(self as &dyn RecommendationsSummaryProvider)
        } else {
            None
        }
    }
    fn as_upgrades_downgrades_provider(&self) -> Option<&dyn UpgradesDowngradesProvider> {
        if self.inner.as_upgrades_downgrades_provider().is_some() {
            Some(self as &dyn UpgradesDowngradesProvider)
        } else {
            None
        }
    }
    fn as_analyst_price_target_provider(&self) -> Option<&dyn AnalystPriceTargetProvider> {
        if self.inner.as_analyst_price_target_provider().is_some() {
            Some(self as &dyn AnalystPriceTargetProvider)
        } else {
            None
        }
    }
    fn as_major_holders_provider(&self) -> Option<&dyn MajorHoldersProvider> {
        if self.inner.as_major_holders_provider().is_some() {
            Some(self as &dyn MajorHoldersProvider)
        } else {
            None
        }
    }
    fn as_institutional_holders_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::InstitutionalHoldersProvider> {
        if self.inner.as_institutional_holders_provider().is_some() {
            Some(self as &dyn borsa_core::connector::InstitutionalHoldersProvider)
        } else {
            None
        }
    }
    fn as_mutual_fund_holders_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::MutualFundHoldersProvider> {
        if self.inner.as_mutual_fund_holders_provider().is_some() {
            Some(self as &dyn borsa_core::connector::MutualFundHoldersProvider)
        } else {
            None
        }
    }
    fn as_insider_transactions_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::InsiderTransactionsProvider> {
        if self.inner.as_insider_transactions_provider().is_some() {
            Some(self as &dyn borsa_core::connector::InsiderTransactionsProvider)
        } else {
            None
        }
    }
    fn as_insider_roster_holders_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::InsiderRosterHoldersProvider> {
        if self.inner.as_insider_roster_holders_provider().is_some() {
            Some(self as &dyn borsa_core::connector::InsiderRosterHoldersProvider)
        } else {
            None
        }
    }
    fn as_net_share_purchase_activity_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::NetSharePurchaseActivityProvider> {
        if self
            .inner
            .as_net_share_purchase_activity_provider()
            .is_some()
        {
            Some(self as &dyn borsa_core::connector::NetSharePurchaseActivityProvider)
        } else {
            None
        }
    }
    fn as_profile_provider(&self) -> Option<&dyn ProfileProvider> {
        if self.inner.as_profile_provider().is_some() {
            Some(self as &dyn ProfileProvider)
        } else {
            None
        }
    }
    fn as_isin_provider(&self) -> Option<&dyn borsa_core::connector::IsinProvider> {
        if self.inner.as_isin_provider().is_some() {
            Some(self as &dyn borsa_core::connector::IsinProvider)
        } else {
            None
        }
    }
    fn as_search_provider(&self) -> Option<&dyn SearchProvider> {
        if self.inner.as_search_provider().is_some() {
            Some(self as &dyn SearchProvider)
        } else {
            None
        }
    }
    fn as_esg_provider(&self) -> Option<&dyn EsgProvider> {
        if self.inner.as_esg_provider().is_some() {
            Some(self as &dyn EsgProvider)
        } else {
            None
        }
    }
    fn as_news_provider(&self) -> Option<&dyn NewsProvider> {
        if self.inner.as_news_provider().is_some() {
            Some(self as &dyn NewsProvider)
        } else {
            None
        }
    }
    fn as_options_expirations_provider(&self) -> Option<&dyn OptionsExpirationsProvider> {
        if self.inner.as_options_expirations_provider().is_some() {
            Some(self as &dyn OptionsExpirationsProvider)
        } else {
            None
        }
    }
    fn as_option_chain_provider(&self) -> Option<&dyn OptionChainProvider> {
        if self.inner.as_option_chain_provider().is_some() {
            Some(self as &dyn OptionChainProvider)
        } else {
            None
        }
    }
    fn as_stream_provider(&self) -> Option<&dyn StreamProvider> {
        if self.inner.as_stream_provider().is_some() {
            Some(self as &dyn StreamProvider)
        } else {
            None
        }
    }
}

#[async_trait]
impl HistoryProvider for BlacklistingMiddleware {
    async fn history(
        &self,
        instrument: &borsa_core::Instrument,
        req: borsa_core::HistoryRequest,
    ) -> Result<borsa_core::HistoryResponse, BorsaError> {
        if self.is_blacklisted() {
            return Err(BorsaError::connector(
                self.name(),
                "provider is temporarily blacklisted",
            ));
        }
        let inner = self
            .inner
            .as_history_provider()
            .ok_or_else(|| BorsaError::unsupported("history"))?;
        inner
            .history(instrument, req)
            .await
            .map_err(|e| self.handle_error(e))
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
impl QuoteProvider for BlacklistingMiddleware {
    async fn quote(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<borsa_core::Quote, BorsaError> {
        if self.is_blacklisted() {
            return Err(BorsaError::connector(
                self.name(),
                "provider is temporarily blacklisted",
            ));
        }
        let inner = self
            .inner
            .as_quote_provider()
            .ok_or_else(|| BorsaError::unsupported("quote"))?;
        inner
            .quote(instrument)
            .await
            .map_err(|e| self.handle_error(e))
    }
}

#[async_trait]
impl EarningsProvider for BlacklistingMiddleware {
    async fn earnings(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<borsa_core::Earnings, BorsaError> {
        if self.is_blacklisted() {
            return Err(BorsaError::connector(
                self.name(),
                "provider is temporarily blacklisted",
            ));
        }
        let inner = self
            .inner
            .as_earnings_provider()
            .ok_or_else(|| BorsaError::unsupported("earnings"))?;
        inner
            .earnings(instrument)
            .await
            .map_err(|e| self.handle_error(e))
    }
}

#[async_trait]
impl IncomeStatementProvider for BlacklistingMiddleware {
    async fn income_statement(
        &self,
        instrument: &borsa_core::Instrument,
        quarterly: bool,
    ) -> Result<Vec<borsa_core::IncomeStatementRow>, BorsaError> {
        if self.is_blacklisted() {
            return Err(BorsaError::connector(
                self.name(),
                "provider is temporarily blacklisted",
            ));
        }
        let inner = self
            .inner
            .as_income_statement_provider()
            .ok_or_else(|| BorsaError::unsupported("income_statement"))?;
        inner
            .income_statement(instrument, quarterly)
            .await
            .map_err(|e| self.handle_error(e))
    }
}

#[async_trait]
impl BalanceSheetProvider for BlacklistingMiddleware {
    async fn balance_sheet(
        &self,
        instrument: &borsa_core::Instrument,
        quarterly: bool,
    ) -> Result<Vec<borsa_core::BalanceSheetRow>, BorsaError> {
        if self.is_blacklisted() {
            return Err(BorsaError::connector(
                self.name(),
                "provider is temporarily blacklisted",
            ));
        }
        let inner = self
            .inner
            .as_balance_sheet_provider()
            .ok_or_else(|| BorsaError::unsupported("balance_sheet"))?;
        inner
            .balance_sheet(instrument, quarterly)
            .await
            .map_err(|e| self.handle_error(e))
    }
}

#[async_trait]
impl CashflowProvider for BlacklistingMiddleware {
    async fn cashflow(
        &self,
        instrument: &borsa_core::Instrument,
        quarterly: bool,
    ) -> Result<Vec<borsa_core::CashflowRow>, BorsaError> {
        if self.is_blacklisted() {
            return Err(BorsaError::connector(
                self.name(),
                "provider is temporarily blacklisted",
            ));
        }
        let inner = self
            .inner
            .as_cashflow_provider()
            .ok_or_else(|| BorsaError::unsupported("cashflow"))?;
        inner
            .cashflow(instrument, quarterly)
            .await
            .map_err(|e| self.handle_error(e))
    }
}

#[async_trait]
impl CalendarProvider for BlacklistingMiddleware {
    async fn calendar(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<borsa_core::Calendar, BorsaError> {
        if self.is_blacklisted() {
            return Err(BorsaError::connector(
                self.name(),
                "provider is temporarily blacklisted",
            ));
        }
        let inner = self
            .inner
            .as_calendar_provider()
            .ok_or_else(|| BorsaError::unsupported("calendar"))?;
        inner
            .calendar(instrument)
            .await
            .map_err(|e| self.handle_error(e))
    }
}

#[async_trait]
impl RecommendationsProvider for BlacklistingMiddleware {
    async fn recommendations(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<Vec<borsa_core::RecommendationRow>, BorsaError> {
        if self.is_blacklisted() {
            return Err(BorsaError::connector(
                self.name(),
                "provider is temporarily blacklisted",
            ));
        }
        let inner = self
            .inner
            .as_recommendations_provider()
            .ok_or_else(|| BorsaError::unsupported("recommendations"))?;
        inner
            .recommendations(instrument)
            .await
            .map_err(|e| self.handle_error(e))
    }
}

#[async_trait]
impl RecommendationsSummaryProvider for BlacklistingMiddleware {
    async fn recommendations_summary(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<borsa_core::RecommendationSummary, BorsaError> {
        if self.is_blacklisted() {
            return Err(BorsaError::connector(
                self.name(),
                "provider is temporarily blacklisted",
            ));
        }
        let inner = self
            .inner
            .as_recommendations_summary_provider()
            .ok_or_else(|| BorsaError::unsupported("recommendations_summary"))?;
        inner
            .recommendations_summary(instrument)
            .await
            .map_err(|e| self.handle_error(e))
    }
}

#[async_trait]
impl UpgradesDowngradesProvider for BlacklistingMiddleware {
    async fn upgrades_downgrades(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<Vec<borsa_core::UpgradeDowngradeRow>, BorsaError> {
        if self.is_blacklisted() {
            return Err(BorsaError::connector(
                self.name(),
                "provider is temporarily blacklisted",
            ));
        }
        let inner = self
            .inner
            .as_upgrades_downgrades_provider()
            .ok_or_else(|| BorsaError::unsupported("upgrades_downgrades"))?;
        inner
            .upgrades_downgrades(instrument)
            .await
            .map_err(|e| self.handle_error(e))
    }
}

#[async_trait]
impl AnalystPriceTargetProvider for BlacklistingMiddleware {
    async fn analyst_price_target(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<borsa_core::PriceTarget, BorsaError> {
        if self.is_blacklisted() {
            return Err(BorsaError::connector(
                self.name(),
                "provider is temporarily blacklisted",
            ));
        }
        let inner = self
            .inner
            .as_analyst_price_target_provider()
            .ok_or_else(|| BorsaError::unsupported("analyst_price_target"))?;
        inner
            .analyst_price_target(instrument)
            .await
            .map_err(|e| self.handle_error(e))
    }
}

#[async_trait]
impl MajorHoldersProvider for BlacklistingMiddleware {
    async fn major_holders(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<Vec<borsa_core::MajorHolder>, BorsaError> {
        if self.is_blacklisted() {
            return Err(BorsaError::connector(
                self.name(),
                "provider is temporarily blacklisted",
            ));
        }
        let inner = self
            .inner
            .as_major_holders_provider()
            .ok_or_else(|| BorsaError::unsupported("major_holders"))?;
        inner
            .major_holders(instrument)
            .await
            .map_err(|e| self.handle_error(e))
    }
}

#[async_trait]
impl borsa_core::connector::InstitutionalHoldersProvider for BlacklistingMiddleware {
    async fn institutional_holders(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<Vec<borsa_core::InstitutionalHolder>, BorsaError> {
        if self.is_blacklisted() {
            return Err(BorsaError::connector(
                self.name(),
                "provider is temporarily blacklisted",
            ));
        }
        let inner = self
            .inner
            .as_institutional_holders_provider()
            .ok_or_else(|| BorsaError::unsupported("institutional_holders"))?;
        inner
            .institutional_holders(instrument)
            .await
            .map_err(|e| self.handle_error(e))
    }
}

#[async_trait]
impl borsa_core::connector::MutualFundHoldersProvider for BlacklistingMiddleware {
    async fn mutual_fund_holders(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<Vec<borsa_core::InstitutionalHolder>, BorsaError> {
        if self.is_blacklisted() {
            return Err(BorsaError::connector(
                self.name(),
                "provider is temporarily blacklisted",
            ));
        }
        let inner = self
            .inner
            .as_mutual_fund_holders_provider()
            .ok_or_else(|| BorsaError::unsupported("mutual_fund_holders"))?;
        inner
            .mutual_fund_holders(instrument)
            .await
            .map_err(|e| self.handle_error(e))
    }
}

#[async_trait]
impl borsa_core::connector::InsiderTransactionsProvider for BlacklistingMiddleware {
    async fn insider_transactions(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<Vec<borsa_core::InsiderTransaction>, BorsaError> {
        if self.is_blacklisted() {
            return Err(BorsaError::connector(
                self.name(),
                "provider is temporarily blacklisted",
            ));
        }
        let inner = self
            .inner
            .as_insider_transactions_provider()
            .ok_or_else(|| BorsaError::unsupported("insider_transactions"))?;
        inner
            .insider_transactions(instrument)
            .await
            .map_err(|e| self.handle_error(e))
    }
}

#[async_trait]
impl borsa_core::connector::InsiderRosterHoldersProvider for BlacklistingMiddleware {
    async fn insider_roster_holders(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<Vec<borsa_core::InsiderRosterHolder>, BorsaError> {
        if self.is_blacklisted() {
            return Err(BorsaError::connector(
                self.name(),
                "provider is temporarily blacklisted",
            ));
        }
        let inner = self
            .inner
            .as_insider_roster_holders_provider()
            .ok_or_else(|| BorsaError::unsupported("insider_roster_holders"))?;
        inner
            .insider_roster_holders(instrument)
            .await
            .map_err(|e| self.handle_error(e))
    }
}

#[async_trait]
impl borsa_core::connector::NetSharePurchaseActivityProvider for BlacklistingMiddleware {
    async fn net_share_purchase_activity(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<Option<borsa_core::NetSharePurchaseActivity>, BorsaError> {
        if self.is_blacklisted() {
            return Err(BorsaError::connector(
                self.name(),
                "provider is temporarily blacklisted",
            ));
        }
        let inner = self
            .inner
            .as_net_share_purchase_activity_provider()
            .ok_or_else(|| BorsaError::unsupported("net_share_purchase_activity"))?;
        inner
            .net_share_purchase_activity(instrument)
            .await
            .map_err(|e| self.handle_error(e))
    }
}

#[async_trait]
impl ProfileProvider for BlacklistingMiddleware {
    async fn profile(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<borsa_core::Profile, BorsaError> {
        if self.is_blacklisted() {
            return Err(BorsaError::connector(
                self.name(),
                "provider is temporarily blacklisted",
            ));
        }
        let inner = self
            .inner
            .as_profile_provider()
            .ok_or_else(|| BorsaError::unsupported("profile"))?;
        inner
            .profile(instrument)
            .await
            .map_err(|e| self.handle_error(e))
    }
}

#[async_trait]
impl borsa_core::connector::IsinProvider for BlacklistingMiddleware {
    async fn isin(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<Option<borsa_core::Isin>, BorsaError> {
        if self.is_blacklisted() {
            return Err(BorsaError::connector(
                self.name(),
                "provider is temporarily blacklisted",
            ));
        }
        let inner = self
            .inner
            .as_isin_provider()
            .ok_or_else(|| BorsaError::unsupported("isin"))?;
        inner
            .isin(instrument)
            .await
            .map_err(|e| self.handle_error(e))
    }
}

#[async_trait]
impl SearchProvider for BlacklistingMiddleware {
    async fn search(
        &self,
        req: borsa_core::SearchRequest,
    ) -> Result<borsa_core::SearchResponse, BorsaError> {
        if self.is_blacklisted() {
            return Err(BorsaError::connector(
                self.name(),
                "provider is temporarily blacklisted",
            ));
        }
        let inner = self
            .inner
            .as_search_provider()
            .ok_or_else(|| BorsaError::unsupported("search"))?;
        inner.search(req).await.map_err(|e| self.handle_error(e))
    }
}

#[async_trait]
impl EsgProvider for BlacklistingMiddleware {
    async fn sustainability(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<borsa_core::EsgScores, BorsaError> {
        if self.is_blacklisted() {
            return Err(BorsaError::connector(
                self.name(),
                "provider is temporarily blacklisted",
            ));
        }
        let inner = self
            .inner
            .as_esg_provider()
            .ok_or_else(|| BorsaError::unsupported("sustainability"))?;
        inner
            .sustainability(instrument)
            .await
            .map_err(|e| self.handle_error(e))
    }
}

#[async_trait]
impl NewsProvider for BlacklistingMiddleware {
    async fn news(
        &self,
        instrument: &borsa_core::Instrument,
        req: borsa_core::NewsRequest,
    ) -> Result<Vec<borsa_core::types::NewsArticle>, BorsaError> {
        if self.is_blacklisted() {
            return Err(BorsaError::connector(
                self.name(),
                "provider is temporarily blacklisted",
            ));
        }
        let inner = self
            .inner
            .as_news_provider()
            .ok_or_else(|| BorsaError::unsupported("news"))?;
        inner
            .news(instrument, req)
            .await
            .map_err(|e| self.handle_error(e))
    }
}

#[async_trait]
impl OptionsExpirationsProvider for BlacklistingMiddleware {
    async fn options_expirations(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<Vec<i64>, BorsaError> {
        if self.is_blacklisted() {
            return Err(BorsaError::connector(
                self.name(),
                "provider is temporarily blacklisted",
            ));
        }
        let inner = self
            .inner
            .as_options_expirations_provider()
            .ok_or_else(|| BorsaError::unsupported("options_expirations"))?;
        inner
            .options_expirations(instrument)
            .await
            .map_err(|e| self.handle_error(e))
    }
}

#[async_trait]
impl OptionChainProvider for BlacklistingMiddleware {
    async fn option_chain(
        &self,
        instrument: &borsa_core::Instrument,
        date: Option<i64>,
    ) -> Result<borsa_core::OptionChain, BorsaError> {
        if self.is_blacklisted() {
            return Err(BorsaError::connector(
                self.name(),
                "provider is temporarily blacklisted",
            ));
        }
        let inner = self
            .inner
            .as_option_chain_provider()
            .ok_or_else(|| BorsaError::unsupported("option_chain"))?;
        inner
            .option_chain(instrument, date)
            .await
            .map_err(|e| self.handle_error(e))
    }
}

#[async_trait]
impl StreamProvider for BlacklistingMiddleware {
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
        if self.is_blacklisted() {
            return Err(BorsaError::connector(
                self.name(),
                "provider is temporarily blacklisted",
            ));
        }
        let inner = self
            .inner
            .as_stream_provider()
            .ok_or_else(|| BorsaError::unsupported("stream_quotes"))?;
        inner
            .stream_quotes(instruments)
            .await
            .map_err(|e| self.handle_error(e))
    }
}
