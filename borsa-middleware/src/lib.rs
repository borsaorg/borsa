//! borsa-middleware
//!
//! Pass-through middleware wrappers around `BorsaConnector` implementations.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use borsa_core::AssetKind;
use borsa_core::connector::{
    AnalystPriceTargetProvider, BalanceSheetProvider, BorsaConnector, CalendarProvider,
    CashflowProvider, EarningsProvider, EsgProvider, HistoryProvider, IncomeStatementProvider,
    NewsProvider, OptionChainProvider, OptionsExpirationsProvider, ProfileProvider, QuoteProvider,
    RecommendationsProvider, RecommendationsSummaryProvider, SearchProvider,
    UpgradesDowngradesProvider,
};
use borsa_types::{QuotaConfig, QuotaState};

/// Wrapper that will enforce quotas (future work). Currently pass-through only.
pub struct QuotaAwareConnector {
    inner: Arc<dyn BorsaConnector>,
    _config: QuotaConfig,
    _state: Mutex<QuotaState>,
}

impl QuotaAwareConnector {
    /// Create a new quota-aware wrapper around an existing connector.
    pub fn new(inner: Arc<dyn BorsaConnector>, config: QuotaConfig, state: QuotaState) -> Self {
        Self {
            inner,
            _config: config,
            _state: Mutex::new(state),
        }
    }

    /// Access the inner connector.
    pub fn inner(&self) -> &Arc<dyn BorsaConnector> {
        &self.inner
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
        self.inner.as_history_provider()
    }
    fn as_quote_provider(&self) -> Option<&dyn QuoteProvider> {
        self.inner.as_quote_provider()
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
