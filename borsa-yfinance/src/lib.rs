//! borsa-yfinance
//!
//! Public connector that implements `BorsaConnector` on top of the `yfinance-rs`
//! client library. Exposes quotes, history, search, fundamentals, options,
//! analysis, holders, ESG, news, and streaming where available.
#![warn(missing_docs)]

/// Adapter definitions and the production adapter backed by `yfinance-rs`.
pub mod adapter;
mod builder;

use std::sync::Arc;
use tokio::sync::mpsc;

#[cfg(feature = "test-adapters")]
use adapter::CloneArcAdapters;
use adapter::{
    RealAdapter, YfAnalysis, YfEsg, YfFundamentals, YfHistory, YfHolders, YfNews, YfOptions,
    YfProfile, YfQuotes, YfSearch, YfStream,
};
use async_trait::async_trait;
use borsa_core::{
    AssetKind, BorsaError, HistoryRequest, HistoryResponse, Instrument, Quote, SearchRequest,
    SearchResponse,
    connector::{
        AnalystPriceTargetProvider, BalanceSheetProvider, BorsaConnector, CalendarProvider,
        CashflowProvider, ConnectorKey, EarningsProvider, EsgProvider, HistoryProvider,
        IncomeStatementProvider, InsiderRosterHoldersProvider, InsiderTransactionsProvider,
        InstitutionalHoldersProvider, IsinProvider, MajorHoldersProvider,
        MutualFundHoldersProvider, NetSharePurchaseActivityProvider, NewsProvider,
        OptionChainProvider, OptionsExpirationsProvider, ProfileProvider, QuoteProvider,
        RecommendationsProvider, RecommendationsSummaryProvider, SearchProvider,
        UpgradesDowngradesProvider,
    },
};

#[cfg(not(feature = "test-adapters"))]
type AdapterArc = Arc<RealAdapter>;

#[cfg(feature = "test-adapters")]
type HistoryAdapter = Arc<dyn YfHistory>;
#[cfg(not(feature = "test-adapters"))]
type HistoryAdapter = AdapterArc;

#[cfg(feature = "test-adapters")]
type QuotesAdapter = Arc<dyn YfQuotes>;
#[cfg(not(feature = "test-adapters"))]
type QuotesAdapter = AdapterArc;

#[cfg(feature = "test-adapters")]
type SearchAdapter = Arc<dyn YfSearch>;
#[cfg(not(feature = "test-adapters"))]
type SearchAdapter = AdapterArc;

#[cfg(feature = "test-adapters")]
type ProfileAdapter = Arc<dyn YfProfile>;
#[cfg(not(feature = "test-adapters"))]
type ProfileAdapter = AdapterArc;

#[cfg(feature = "test-adapters")]
type FundamentalsAdapter = Arc<dyn YfFundamentals>;
#[cfg(not(feature = "test-adapters"))]
type FundamentalsAdapter = AdapterArc;

#[cfg(feature = "test-adapters")]
type OptionsAdapter = Arc<dyn YfOptions>;
#[cfg(not(feature = "test-adapters"))]
type OptionsAdapter = AdapterArc;

#[cfg(feature = "test-adapters")]
type AnalysisAdapter = Arc<dyn YfAnalysis>;
#[cfg(not(feature = "test-adapters"))]
type AnalysisAdapter = AdapterArc;

#[cfg(feature = "test-adapters")]
type HoldersAdapter = Arc<dyn YfHolders>;
#[cfg(not(feature = "test-adapters"))]
type HoldersAdapter = AdapterArc;

#[cfg(feature = "test-adapters")]
type EsgAdapter = Arc<dyn YfEsg>;
#[cfg(not(feature = "test-adapters"))]
type EsgAdapter = AdapterArc;

#[cfg(feature = "test-adapters")]
type NewsAdapter = Arc<dyn YfNews>;
#[cfg(not(feature = "test-adapters"))]
type NewsAdapter = AdapterArc;

#[cfg(feature = "test-adapters")]
type StreamAdapter = Arc<dyn YfStream>;
#[cfg(not(feature = "test-adapters"))]
type StreamAdapter = AdapterArc;

/// Public connector type. Production users will construct with `YfConnector::new_default()`.
pub struct YfConnector {
    history: HistoryAdapter,
    quotes: QuotesAdapter,
    search: SearchAdapter,
    profile: ProfileAdapter,
    fundamentals: FundamentalsAdapter,
    options: OptionsAdapter,
    analysis: AnalysisAdapter,
    holders: HoldersAdapter,
    esg: EsgAdapter,
    news: NewsAdapter,
    stream: StreamAdapter,
}

impl YfConnector {
    /// Static connector key for orchestrator priority configuration.
    #[must_use]
    pub const fn key_static() -> ConnectorKey {
        ConnectorKey::new("borsa-yfinance")
    }

    fn looks_like_not_found(msg: &str) -> bool {
        let m = msg.to_ascii_lowercase();
        m.contains("not found") || m.contains("no data") || m.contains("no matches")
    }

    fn normalize_error(e: BorsaError, what: &str) -> BorsaError {
        match e {
            BorsaError::Connector { connector: _, msg } => {
                if Self::looks_like_not_found(&msg) {
                    BorsaError::not_found(what.to_string())
                } else {
                    BorsaError::connector("borsa-yfinance", msg)
                }
            }
            BorsaError::Other(msg) => BorsaError::connector("borsa-yfinance", msg),
            other => other,
        }
    }
    /// Build with a fresh `yfinance_rs::YfClient` inside.
    #[must_use]
    pub fn new_default() -> Self {
        let a = RealAdapter::new_default();
        Self::from_adapter(&a)
    }

    /// Build from an existing `yfinance_rs::YfClient`.
    #[must_use]
    pub fn new_with_client(client: yfinance_rs::YfClient) -> Self {
        let a = RealAdapter::new(client);
        Self::from_adapter(&a)
    }

    /// Build from a provided `reqwest::Client` by constructing a `yfinance_rs::YfClient`.
    ///
    /// Note: The provided client should enable a cookie store for yfinance auth/crumb flow.
    ///
    /// # Errors
    /// Returns an error if the internal `YfClient` cannot be constructed from the provided HTTP client.
    pub fn try_new_with_reqwest_client(
        http: reqwest::Client,
    ) -> Result<Self, borsa_core::BorsaError> {
        let yf = yfinance_rs::YfClient::builder()
            .custom_client(http)
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
            .build()
            .map_err(|e| borsa_core::BorsaError::Other(e.to_string()))?;
        Ok(Self::new_with_client(yf))
    }

    /// For tests/injection (requires the `test-adapters` feature).
    ///
    /// Accepts a borrowed adapter to avoid unnecessary moves.
    #[cfg(feature = "test-adapters")]
    pub fn from_adapter<A: CloneArcAdapters + 'static>(adapter: &A) -> Self {
        Self {
            history: adapter.clone_arc_history(),
            quotes: adapter.clone_arc_quotes(),
            search: adapter.clone_arc_search(),
            profile: adapter.clone_arc_profile(),
            fundamentals: adapter.clone_arc_fundamentals(),
            options: adapter.clone_arc_options(),
            analysis: adapter.clone_arc_analysis(),
            holders: adapter.clone_arc_holders(),
            esg: adapter.clone_arc_esg(),
            news: adapter.clone_arc_news(),
            stream: adapter.clone_arc_stream(),
        }
    }

    #[cfg(not(feature = "test-adapters"))]
    /// Build from a concrete `RealAdapter` by cloning it into shared handles.
    pub fn from_adapter(adapter: &RealAdapter) -> Self {
        let shared = Arc::new(adapter.clone());
        Self {
            history: Arc::clone(&shared),
            quotes: Arc::clone(&shared),
            search: Arc::clone(&shared),
            profile: Arc::clone(&shared),
            fundamentals: Arc::clone(&shared),
            options: Arc::clone(&shared),
            analysis: Arc::clone(&shared),
            holders: Arc::clone(&shared),
            esg: Arc::clone(&shared),
            news: Arc::clone(&shared),
            stream: shared,
        }
    }
}

#[async_trait]
impl QuoteProvider for YfConnector {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa_yfinance::quote",
            skip(self, instrument),
            fields(symbol = %instrument.symbol()),
        )
    )]
    async fn quote(&self, instrument: &Instrument) -> Result<Quote, BorsaError> {
        let raw = self
            .quotes
            .fetch(std::slice::from_ref(&instrument.symbol().to_string()))
            .await
            .map_err(|e| Self::normalize_error(e, &format!("quote for {}", instrument.symbol())))?;
        let first = raw
            .into_iter()
            .next()
            .ok_or_else(|| BorsaError::not_found(format!("quote for {}", instrument.symbol())))?;
        Ok(first)
    }
}

#[async_trait]
impl HistoryProvider for YfConnector {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa_yfinance::history",
            skip(self, instrument, req),
            fields(symbol = %instrument.symbol(), interval = ?req.interval(), range = ?req.range(), include_prepost = req.include_prepost(), include_actions = req.include_actions(), auto_adjust = req.auto_adjust(), keepna = req.keepna()),
        )
    )]
    async fn history(
        &self,
        instrument: &Instrument,
        req: HistoryRequest,
    ) -> Result<HistoryResponse, BorsaError> {
        let yf_req = yfinance_rs::core::services::HistoryRequest {
            range: req.range(),
            period: req.period().map(|(s, e)| (s.timestamp(), e.timestamp())),
            interval: req.interval(),
            include_prepost: req.include_prepost(),
            include_actions: req.include_actions(),
            auto_adjust: req.auto_adjust(),
            keepna: req.keepna(),
        };
        let symbol = instrument.symbol_str();
        let raw = self.history.fetch_full(symbol, yf_req).await?;
        Ok(raw)
    }

    fn supported_history_intervals(
        &self,
        _kind: AssetKind,
    ) -> &'static [borsa_core::types::Interval] {
        use borsa_core::types::Interval as I;
        const YF_INTERVALS: &[I] = &[
            I::I1m,
            I::I2m,
            I::I5m,
            I::I15m,
            I::I30m,
            I::I1h,
            I::I90m,
            I::D1,
            I::D5,
            I::W1,
            I::M1,
            I::M3,
        ];
        YF_INTERVALS
    }
}

#[async_trait]
impl ProfileProvider for YfConnector {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa_yfinance::profile",
            skip(self, instrument),
            fields(symbol = %instrument.symbol()),
        )
    )]
    async fn profile(&self, instrument: &Instrument) -> Result<borsa_core::Profile, BorsaError> {
        let symbol = instrument.symbol_str();
        let raw = self.profile.load(symbol).await?;
        Ok(raw)
    }
}

#[async_trait]
impl IsinProvider for YfConnector {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa_yfinance::isin",
            skip(self, instrument),
            fields(symbol = %instrument.symbol()),
        )
    )]
    async fn isin(&self, instrument: &Instrument) -> Result<Option<borsa_core::Isin>, BorsaError> {
        let symbol = instrument.symbol_str();
        match self.profile.isin(symbol).await {
            Ok(Some(isin_str)) => match borsa_core::Isin::new(&isin_str) {
                Ok(isin) => Ok(Some(isin)),
                Err(e) => Err(BorsaError::Data(format!(
                    "Provider returned invalid ISIN '{isin_str}': {e}"
                ))),
            },
            Ok(None) => Ok(None),
            Err(e) => Err(Self::normalize_error(e, &format!("isin for {symbol}"))),
        }
    }
}

#[async_trait]
impl SearchProvider for YfConnector {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa_yfinance::search",
            skip(self, req),
            fields(kind = ?req.kind(), limit = req.limit()),
        )
    )]
    async fn search(&self, req: SearchRequest) -> Result<SearchResponse, BorsaError> {
        self.search.search(&req).await
    }
}

#[async_trait]
impl EarningsProvider for YfConnector {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa_yfinance::earnings",
            skip(self, instrument),
            fields(symbol = %instrument.symbol()),
        )
    )]
    async fn earnings(&self, instrument: &Instrument) -> Result<borsa_core::Earnings, BorsaError> {
        let symbol = instrument.symbol_str();
        let raw = self.fundamentals.earnings(symbol).await?;
        Ok(raw)
    }
}

#[async_trait]
impl IncomeStatementProvider for YfConnector {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa_yfinance::income_statement",
            skip(self, instrument),
            fields(symbol = %instrument.symbol(), quarterly = quarterly),
        )
    )]
    async fn income_statement(
        &self,
        instrument: &Instrument,
        quarterly: bool,
    ) -> Result<Vec<borsa_core::IncomeStatementRow>, BorsaError> {
        let raw = self
            .fundamentals
            .income_statement(instrument.symbol_str(), quarterly)
            .await?;
        Ok(raw)
    }
}

#[async_trait]
impl BalanceSheetProvider for YfConnector {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa_yfinance::balance_sheet",
            skip(self, instrument),
            fields(symbol = %instrument.symbol(), quarterly = quarterly),
        )
    )]
    async fn balance_sheet(
        &self,
        instrument: &Instrument,
        quarterly: bool,
    ) -> Result<Vec<borsa_core::BalanceSheetRow>, BorsaError> {
        let raw = self
            .fundamentals
            .balance_sheet(instrument.symbol_str(), quarterly)
            .await?;
        Ok(raw)
    }
}

#[async_trait]
impl CashflowProvider for YfConnector {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa_yfinance::cashflow",
            skip(self, instrument),
            fields(symbol = %instrument.symbol(), quarterly = quarterly),
        )
    )]
    async fn cashflow(
        &self,
        instrument: &Instrument,
        quarterly: bool,
    ) -> Result<Vec<borsa_core::CashflowRow>, BorsaError> {
        let raw = self
            .fundamentals
            .cashflow(instrument.symbol_str(), quarterly)
            .await?;
        Ok(raw)
    }
}

#[async_trait]
impl CalendarProvider for YfConnector {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa_yfinance::calendar",
            skip(self, instrument),
            fields(symbol = %instrument.symbol()),
        )
    )]
    async fn calendar(&self, instrument: &Instrument) -> Result<borsa_core::Calendar, BorsaError> {
        let raw = self.fundamentals.calendar(instrument.symbol_str()).await?;
        Ok(raw)
    }
}

#[async_trait]
impl OptionsExpirationsProvider for YfConnector {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa_yfinance::options_expirations",
            skip(self, instrument),
            fields(symbol = %instrument.symbol()),
        )
    )]
    async fn options_expirations(&self, instrument: &Instrument) -> Result<Vec<i64>, BorsaError> {
        self.options.expirations(instrument.symbol_str()).await
    }
}

#[async_trait]
impl OptionChainProvider for YfConnector {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa_yfinance::option_chain",
            skip(self, instrument),
            fields(symbol = %instrument.symbol(), date = ?date),
        )
    )]
    async fn option_chain(
        &self,
        instrument: &Instrument,
        date: Option<i64>,
    ) -> Result<borsa_core::OptionChain, BorsaError> {
        let raw = self.options.chain(instrument.symbol_str(), date).await?;
        Ok(raw)
    }
}

#[async_trait]
impl RecommendationsProvider for YfConnector {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa_yfinance::recommendations",
            skip(self, instrument),
            fields(symbol = %instrument.symbol()),
        )
    )]
    async fn recommendations(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<borsa_core::RecommendationRow>, BorsaError> {
        let rows = self
            .analysis
            .recommendations(instrument.symbol_str())
            .await?;
        Ok(rows)
    }
}

#[async_trait]
impl RecommendationsSummaryProvider for YfConnector {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa_yfinance::recommendations_summary",
            skip(self, instrument),
            fields(symbol = %instrument.symbol()),
        )
    )]
    async fn recommendations_summary(
        &self,
        instrument: &Instrument,
    ) -> Result<borsa_core::RecommendationSummary, BorsaError> {
        let s = self
            .analysis
            .recommendations_summary(instrument.symbol_str())
            .await?;
        Ok(s)
    }
}

#[async_trait]
impl UpgradesDowngradesProvider for YfConnector {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa_yfinance::upgrades_downgrades",
            skip(self, instrument),
            fields(symbol = %instrument.symbol()),
        )
    )]
    async fn upgrades_downgrades(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<borsa_core::UpgradeDowngradeRow>, BorsaError> {
        let v = self
            .analysis
            .upgrades_downgrades(instrument.symbol_str())
            .await?;
        Ok(v)
    }
}

#[async_trait]
impl AnalystPriceTargetProvider for YfConnector {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa_yfinance::analyst_price_target",
            skip(self, instrument),
            fields(symbol = %instrument.symbol()),
        )
    )]
    async fn analyst_price_target(
        &self,
        instrument: &Instrument,
    ) -> Result<borsa_core::PriceTarget, BorsaError> {
        let p = self
            .analysis
            .analyst_price_target(instrument.symbol_str())
            .await?;
        Ok(p)
    }
}

#[async_trait]
impl MajorHoldersProvider for YfConnector {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa_yfinance::major_holders",
            skip(self, instrument),
            fields(symbol = %instrument.symbol()),
        )
    )]
    async fn major_holders(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<borsa_core::MajorHolder>, BorsaError> {
        let rows = self.holders.major_holders(instrument.symbol_str()).await?;
        let mapped = rows.into_iter().collect();
        Ok(mapped)
    }
}

#[async_trait]
impl InstitutionalHoldersProvider for YfConnector {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa_yfinance::institutional_holders",
            skip(self, instrument),
            fields(symbol = %instrument.symbol()),
        )
    )]
    async fn institutional_holders(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<borsa_core::InstitutionalHolder>, BorsaError> {
        let rows = self
            .holders
            .institutional_holders(instrument.symbol_str())
            .await?;
        Ok(rows)
    }
}

#[async_trait]
impl MutualFundHoldersProvider for YfConnector {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa_yfinance::mutual_fund_holders",
            skip(self, instrument),
            fields(symbol = %instrument.symbol()),
        )
    )]
    async fn mutual_fund_holders(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<borsa_core::InstitutionalHolder>, BorsaError> {
        let rows = self
            .holders
            .mutual_fund_holders(instrument.symbol_str())
            .await?;
        Ok(rows)
    }
}

#[async_trait]
impl InsiderTransactionsProvider for YfConnector {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa_yfinance::insider_transactions",
            skip(self, instrument),
            fields(symbol = %instrument.symbol()),
        )
    )]
    async fn insider_transactions(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<borsa_core::InsiderTransaction>, BorsaError> {
        let rows = self
            .holders
            .insider_transactions(instrument.symbol_str())
            .await?;
        Ok(rows)
    }
}

#[async_trait]
impl InsiderRosterHoldersProvider for YfConnector {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa_yfinance::insider_roster_holders",
            skip(self, instrument),
            fields(symbol = %instrument.symbol()),
        )
    )]
    async fn insider_roster_holders(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<borsa_core::InsiderRosterHolder>, BorsaError> {
        let rows = self
            .holders
            .insider_roster_holders(instrument.symbol_str())
            .await?;
        Ok(rows)
    }
}

#[async_trait]
impl NetSharePurchaseActivityProvider for YfConnector {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa_yfinance::net_share_purchase_activity",
            skip(self, instrument),
            fields(symbol = %instrument.symbol()),
        )
    )]
    async fn net_share_purchase_activity(
        &self,
        instrument: &Instrument,
    ) -> Result<Option<borsa_core::NetSharePurchaseActivity>, BorsaError> {
        let activity = self
            .holders
            .net_share_purchase_activity(instrument.symbol_str())
            .await?;
        Ok(activity)
    }
}

// Currently disabled because the endpoint disappeared from the Yahoo Finance API.
#[async_trait]
impl EsgProvider for YfConnector {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa_yfinance::sustainability",
            skip(self, instrument),
            fields(symbol = %instrument.symbol()),
        )
    )]
    async fn sustainability(
        &self,
        instrument: &Instrument,
    ) -> Result<borsa_core::EsgScores, BorsaError> {
        let scores = self.esg.sustainability(instrument.symbol_str()).await?;
        Ok(scores)
    }
}

#[async_trait]
impl NewsProvider for YfConnector {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa_yfinance::news",
            skip(self, instrument, req),
            fields(symbol = %instrument.symbol()),
        )
    )]
    async fn news(
        &self,
        instrument: &Instrument,
        req: borsa_core::NewsRequest,
    ) -> Result<Vec<borsa_core::types::NewsArticle>, BorsaError> {
        let articles = self.news.news(instrument.symbol_str(), req).await?;
        Ok(articles)
    }
}

#[async_trait]
impl borsa_core::connector::StreamProvider for YfConnector {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa_yfinance::stream_quotes",
            skip(self, instruments),
            fields(num_symbols = instruments.len()),
        )
    )]
    async fn stream_quotes(
        &self,
        instruments: &[Instrument],
    ) -> Result<
        (
            borsa_core::stream::StreamHandle,
            mpsc::Receiver<borsa_core::QuoteUpdate>,
        ),
        BorsaError,
    > {
        let symbols: Vec<String> = instruments.iter().map(|i| i.symbol().to_string()).collect();
        self.stream.start(&symbols).await
    }
}

#[async_trait]
impl BorsaConnector for YfConnector {
    fn name(&self) -> &'static str {
        "borsa-yfinance"
    }
    fn vendor(&self) -> &'static str {
        "Yahoo Finance"
    }

    // capabilities removed; capability directory via as_*_provider

    fn as_history_provider(&self) -> Option<&dyn borsa_core::connector::HistoryProvider> {
        Some(self as &dyn HistoryProvider)
    }

    fn as_quote_provider(&self) -> Option<&dyn borsa_core::connector::QuoteProvider> {
        Some(self as &dyn QuoteProvider)
    }

    fn as_stream_provider(&self) -> Option<&dyn borsa_core::connector::StreamProvider> {
        Some(self as &dyn borsa_core::connector::StreamProvider)
    }

    fn as_profile_provider(&self) -> Option<&dyn borsa_core::connector::ProfileProvider> {
        Some(self as &dyn ProfileProvider)
    }

    fn as_isin_provider(&self) -> Option<&dyn borsa_core::connector::IsinProvider> {
        Some(self as &dyn IsinProvider)
    }

    fn as_search_provider(&self) -> Option<&dyn borsa_core::connector::SearchProvider> {
        Some(self as &dyn SearchProvider)
    }

    fn as_esg_provider(&self) -> Option<&dyn borsa_core::connector::EsgProvider> {
        None
        // Currently disabled because the endpoint disappeared from the Yahoo Finance API.
        // Some(self as &dyn EsgProvider)
    }

    fn as_news_provider(&self) -> Option<&dyn borsa_core::connector::NewsProvider> {
        Some(self as &dyn NewsProvider)
    }

    fn as_earnings_provider(&self) -> Option<&dyn borsa_core::connector::EarningsProvider> {
        Some(self as &dyn EarningsProvider)
    }
    fn as_income_statement_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::IncomeStatementProvider> {
        Some(self as &dyn IncomeStatementProvider)
    }
    fn as_balance_sheet_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::BalanceSheetProvider> {
        Some(self as &dyn BalanceSheetProvider)
    }
    fn as_cashflow_provider(&self) -> Option<&dyn borsa_core::connector::CashflowProvider> {
        Some(self as &dyn CashflowProvider)
    }
    fn as_calendar_provider(&self) -> Option<&dyn borsa_core::connector::CalendarProvider> {
        Some(self as &dyn CalendarProvider)
    }

    fn as_recommendations_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::RecommendationsProvider> {
        Some(self as &dyn RecommendationsProvider)
    }
    fn as_recommendations_summary_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::RecommendationsSummaryProvider> {
        Some(self as &dyn RecommendationsSummaryProvider)
    }
    fn as_upgrades_downgrades_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::UpgradesDowngradesProvider> {
        Some(self as &dyn UpgradesDowngradesProvider)
    }
    fn as_analyst_price_target_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::AnalystPriceTargetProvider> {
        Some(self as &dyn AnalystPriceTargetProvider)
    }

    fn as_major_holders_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::MajorHoldersProvider> {
        Some(self as &dyn MajorHoldersProvider)
    }
    fn as_institutional_holders_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::InstitutionalHoldersProvider> {
        Some(self as &dyn InstitutionalHoldersProvider)
    }
    fn as_mutual_fund_holders_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::MutualFundHoldersProvider> {
        Some(self as &dyn MutualFundHoldersProvider)
    }
    fn as_insider_transactions_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::InsiderTransactionsProvider> {
        Some(self as &dyn InsiderTransactionsProvider)
    }
    fn as_insider_roster_holders_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::InsiderRosterHoldersProvider> {
        Some(self as &dyn InsiderRosterHoldersProvider)
    }
    fn as_net_share_purchase_activity_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::NetSharePurchaseActivityProvider> {
        Some(self as &dyn NetSharePurchaseActivityProvider)
    }

    fn as_options_expirations_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::OptionsExpirationsProvider> {
        Some(self as &dyn OptionsExpirationsProvider)
    }
    fn as_option_chain_provider(&self) -> Option<&dyn borsa_core::connector::OptionChainProvider> {
        Some(self as &dyn OptionChainProvider)
    }

    /// yfinance is fairly broad; we default to `true` and let router priorities steer quality.
    fn supports_kind(&self, kind: AssetKind) -> bool {
        matches!(
            kind,
            AssetKind::Equity
                | AssetKind::Fund
                | AssetKind::Index
                | AssetKind::Crypto
                | AssetKind::Forex
        )
    }
}
