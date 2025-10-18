use async_trait::async_trait;
use borsa_core::connector::{
    AnalystPriceTargetProvider, BalanceSheetProvider, BorsaConnector, CalendarProvider,
    CashflowProvider, EarningsProvider, EsgProvider, HistoryProvider, IncomeStatementProvider,
    NewsProvider, OptionChainProvider, OptionsExpirationsProvider, ProfileProvider, QuoteProvider,
    RecommendationsProvider, RecommendationsSummaryProvider, SearchProvider,
    UpgradesDowngradesProvider,
};
use borsa_core::{
    AssetKind, BalanceSheetRow, BorsaError, Calendar, CashflowRow, Earnings, EsgScores,
    HistoryRequest, HistoryResponse, IncomeStatementRow, Instrument, Interval, NewsRequest,
    OptionChain, Profile, Quote, RecommendationRow, RecommendationSummary, SearchRequest,
    SearchResponse, UpgradeDowngradeRow, types,
};

mod fixtures;

/// Mock connector for CI-safe examples. Provides deterministic data from static fixtures.
pub struct MockConnector;

impl Default for MockConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl MockConnector {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    fn not_found(what: &str) -> BorsaError {
        BorsaError::not_found(what.to_string())
    }

    fn maybe_fail_or_timeout(symbol: &str, capability: &'static str) -> Result<(), BorsaError> {
        match symbol {
            "FAIL" => Err(BorsaError::connector(
                "borsa-mock",
                format!("forced failure: {capability}"),
            )),
            "TIMEOUT" => {
                // Simulate brief latency; orchestrator may time out depending on config
                // Keep short to avoid slowing tests excessively
                let () = std::thread::sleep(std::time::Duration::from_millis(200));
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

#[async_trait]
impl BorsaConnector for MockConnector {
    fn name(&self) -> &'static str {
        "borsa-mock"
    }
    fn vendor(&self) -> &'static str {
        "Mock"
    }

    fn supports_kind(&self, _kind: AssetKind) -> bool {
        true
    }

    fn as_quote_provider(&self) -> Option<&dyn QuoteProvider> {
        Some(self as &dyn QuoteProvider)
    }
    fn as_history_provider(&self) -> Option<&dyn HistoryProvider> {
        Some(self as &dyn HistoryProvider)
    }
    fn as_search_provider(&self) -> Option<&dyn SearchProvider> {
        Some(self as &dyn SearchProvider)
    }
    fn as_profile_provider(&self) -> Option<&dyn ProfileProvider> {
        Some(self as &dyn ProfileProvider)
    }
    fn as_earnings_provider(&self) -> Option<&dyn EarningsProvider> {
        Some(self as &dyn EarningsProvider)
    }
    fn as_income_statement_provider(&self) -> Option<&dyn IncomeStatementProvider> {
        Some(self as &dyn IncomeStatementProvider)
    }
    fn as_balance_sheet_provider(&self) -> Option<&dyn BalanceSheetProvider> {
        Some(self as &dyn BalanceSheetProvider)
    }
    fn as_cashflow_provider(&self) -> Option<&dyn CashflowProvider> {
        Some(self as &dyn CashflowProvider)
    }
    fn as_calendar_provider(&self) -> Option<&dyn CalendarProvider> {
        Some(self as &dyn CalendarProvider)
    }
    fn as_options_expirations_provider(&self) -> Option<&dyn OptionsExpirationsProvider> {
        Some(self as &dyn OptionsExpirationsProvider)
    }
    fn as_option_chain_provider(&self) -> Option<&dyn OptionChainProvider> {
        Some(self as &dyn OptionChainProvider)
    }
    fn as_recommendations_provider(&self) -> Option<&dyn RecommendationsProvider> {
        Some(self as &dyn RecommendationsProvider)
    }
    fn as_recommendations_summary_provider(&self) -> Option<&dyn RecommendationsSummaryProvider> {
        Some(self as &dyn RecommendationsSummaryProvider)
    }
    fn as_upgrades_downgrades_provider(&self) -> Option<&dyn UpgradesDowngradesProvider> {
        Some(self as &dyn UpgradesDowngradesProvider)
    }
    fn as_analyst_price_target_provider(&self) -> Option<&dyn AnalystPriceTargetProvider> {
        Some(self as &dyn AnalystPriceTargetProvider)
    }
    fn as_esg_provider(&self) -> Option<&dyn EsgProvider> {
        Some(self as &dyn EsgProvider)
    }
    fn as_news_provider(&self) -> Option<&dyn NewsProvider> {
        Some(self as &dyn NewsProvider)
    }
    // Stream intentionally unsupported for examples
}

#[async_trait]
impl QuoteProvider for MockConnector {
    async fn quote(&self, instrument: &Instrument) -> Result<Quote, BorsaError> {
        let s = instrument.symbol_str();
        Self::maybe_fail_or_timeout(s, "quote")?;
        fixtures::quotes::by_symbol(s).ok_or_else(|| Self::not_found(&format!("quote for {s}")))
    }
}

#[async_trait]
impl HistoryProvider for MockConnector {
    async fn history(
        &self,
        instrument: &Instrument,
        _req: HistoryRequest,
    ) -> Result<HistoryResponse, BorsaError> {
        let s = instrument.symbol_str();
        Self::maybe_fail_or_timeout(s, "history")?;
        fixtures::history::by_symbol(s).ok_or_else(|| Self::not_found(&format!("history for {s}")))
    }

    fn supported_history_intervals(&self, _kind: AssetKind) -> &'static [Interval] {
        const ONLY_D1: &[Interval] = &[Interval::D1];
        ONLY_D1
    }
}

#[async_trait]
impl SearchProvider for MockConnector {
    async fn search(&self, req: SearchRequest) -> Result<SearchResponse, BorsaError> {
        Ok(fixtures::search::search(&req))
    }
}

#[async_trait]
impl ProfileProvider for MockConnector {
    async fn profile(&self, instrument: &Instrument) -> Result<Profile, BorsaError> {
        let s = instrument.symbol_str();
        fixtures::profile::by_symbol(s).ok_or_else(|| Self::not_found(&format!("profile for {s}")))
    }
}

#[async_trait]
impl EarningsProvider for MockConnector {
    async fn earnings(&self, instrument: &Instrument) -> Result<Earnings, BorsaError> {
        let s = instrument.symbol_str();
        fixtures::fundamentals::earnings_by_symbol(s)
            .ok_or_else(|| Self::not_found(&format!("earnings for {s}")))
    }
}

#[async_trait]
impl IncomeStatementProvider for MockConnector {
    async fn income_statement(
        &self,
        instrument: &Instrument,
        _q: bool,
    ) -> Result<Vec<IncomeStatementRow>, BorsaError> {
        let s = instrument.symbol_str();
        Ok(fixtures::fundamentals::income_stmt_by_symbol(s))
    }
}

#[async_trait]
impl BalanceSheetProvider for MockConnector {
    async fn balance_sheet(
        &self,
        instrument: &Instrument,
        _q: bool,
    ) -> Result<Vec<BalanceSheetRow>, BorsaError> {
        let s = instrument.symbol_str();
        Ok(fixtures::fundamentals::balance_sheet_by_symbol(s))
    }
}

#[async_trait]
impl CashflowProvider for MockConnector {
    async fn cashflow(
        &self,
        instrument: &Instrument,
        _q: bool,
    ) -> Result<Vec<CashflowRow>, BorsaError> {
        let s = instrument.symbol_str();
        Ok(fixtures::fundamentals::cashflow_by_symbol(s))
    }
}

#[async_trait]
impl CalendarProvider for MockConnector {
    async fn calendar(&self, instrument: &Instrument) -> Result<Calendar, BorsaError> {
        let s = instrument.symbol_str();
        fixtures::calendar::by_symbol(s)
            .ok_or_else(|| Self::not_found(&format!("calendar for {s}")))
    }
}

#[async_trait]
impl OptionsExpirationsProvider for MockConnector {
    async fn options_expirations(&self, instrument: &Instrument) -> Result<Vec<i64>, BorsaError> {
        let s = instrument.symbol_str();
        Ok(fixtures::options::expirations_by_symbol(s))
    }
}

#[async_trait]
impl OptionChainProvider for MockConnector {
    async fn option_chain(
        &self,
        instrument: &Instrument,
        date: Option<i64>,
    ) -> Result<OptionChain, BorsaError> {
        let s = instrument.symbol_str();
        fixtures::options::chain_by_symbol_and_date(s, date)
            .ok_or_else(|| Self::not_found(&format!("option chain for {s}")))
    }
}

#[async_trait]
impl RecommendationsProvider for MockConnector {
    async fn recommendations(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<RecommendationRow>, BorsaError> {
        let s = instrument.symbol_str();
        Ok(fixtures::analysis::recommendations_by_symbol(s))
    }
}

#[async_trait]
impl RecommendationsSummaryProvider for MockConnector {
    async fn recommendations_summary(
        &self,
        instrument: &Instrument,
    ) -> Result<RecommendationSummary, BorsaError> {
        let s = instrument.symbol_str();
        fixtures::analysis::recommendations_summary_by_symbol(s)
            .ok_or_else(|| Self::not_found(&format!("recommendations summary for {s}")))
    }
}

#[async_trait]
impl AnalystPriceTargetProvider for MockConnector {
    async fn analyst_price_target(
        &self,
        instrument: &Instrument,
    ) -> Result<borsa_core::PriceTarget, BorsaError> {
        let _s = instrument.symbol_str();
        Ok(fixtures::analysis::price_target_by_symbol(_s))
    }
}

#[async_trait]
impl UpgradesDowngradesProvider for MockConnector {
    async fn upgrades_downgrades(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<UpgradeDowngradeRow>, BorsaError> {
        let s = instrument.symbol_str();
        Ok(fixtures::analysis::upgrades_downgrades_by_symbol(s))
    }
}

#[async_trait]
impl EsgProvider for MockConnector {
    async fn sustainability(&self, instrument: &Instrument) -> Result<EsgScores, BorsaError> {
        let s = instrument.symbol_str();
        fixtures::esg::by_symbol(s)
            .ok_or_else(|| Self::not_found(&format!("sustainability for {s}")))
    }
}

#[async_trait]
impl NewsProvider for MockConnector {
    async fn news(
        &self,
        instrument: &Instrument,
        req: NewsRequest,
    ) -> Result<Vec<types::NewsArticle>, BorsaError> {
        let s = instrument.symbol_str();
        Ok(fixtures::news::by_symbol(s, &req))
    }
}
