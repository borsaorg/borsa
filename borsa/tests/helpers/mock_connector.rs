#![allow(dead_code)]
#![allow(clippy::type_complexity)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::cast_possible_truncation)]

use std::sync::Arc;

use async_trait::async_trait;
use borsa_core::{
    AssetKind, BalanceSheetRow, BorsaConnector, BorsaError, Calendar, Candle, CashflowRow,
    EsgScores, HistoryRequest, HistoryResponse, IncomeStatementRow, Instrument, MajorHolder,
    NewsArticle, OptionChain, PriceTarget, Quote, RecommendationRow, RecommendationSummary,
    UpgradeDowngradeRow,
    connector::{
        AnalystPriceTargetProvider, BalanceSheetProvider, CalendarProvider, CashflowProvider,
        EsgProvider, HistoryProvider, IncomeStatementProvider, MajorHoldersProvider, NewsProvider,
        OptionChainProvider, OptionsExpirationsProvider, QuoteProvider, RecommendationsProvider,
        RecommendationsSummaryProvider, SearchProvider, StreamProvider, UpgradesDowngradesProvider,
    },
};
use borsa_core::{NewsRequest, SearchRequest, SearchResponse, SearchResult};
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};

/// Simple in-memory connector used by integration tests.
/// You can tailor behavior (success/fail, supported kinds, etc.) via fields below.
const DEFAULT_HISTORY_INTERVALS: &[borsa_core::Interval] = &[
    borsa_core::Interval::I1m,
    borsa_core::Interval::I2m,
    borsa_core::Interval::I15m,
    borsa_core::Interval::I30m,
    borsa_core::Interval::I1h,
    borsa_core::Interval::I90m,
    borsa_core::Interval::D1,
    borsa_core::Interval::W1,
];

pub struct MockConnector {
    pub name: &'static str,
    pub kind_ok: Option<AssetKind>,
    pub quote: Option<Quote>,
    pub history: Option<HistoryResponse>,
    pub search: Option<Vec<SearchResult>>,
    pub delay_ms: u64,
    pub stream_updates: Option<Vec<borsa_core::QuoteUpdate>>,
    pub stream_start_error: Option<&'static str>,
    pub history_intervals: &'static [borsa_core::Interval],
    // Optional scripted steps applied per stream_quotes call (in order).
    pub stream_steps: Option<Arc<Mutex<Vec<StreamStep>>>>,

    // Optional closures to customize behavior per test
    pub quote_fn: Option<Arc<dyn Fn(&Instrument) -> Result<Quote, BorsaError> + Send + Sync>>,
    pub history_fn: Option<
        Arc<
            dyn Fn(&Instrument, HistoryRequest) -> Result<HistoryResponse, BorsaError>
                + Send
                + Sync,
        >,
    >,
    pub search_fn:
        Option<Arc<dyn Fn(SearchRequest) -> Result<SearchResponse, BorsaError> + Send + Sync>>,
    pub calendar_fn: Option<Arc<dyn Fn(&Instrument) -> Result<Calendar, BorsaError> + Send + Sync>>,
    pub balance_sheet_fn: Option<
        Arc<dyn Fn(&Instrument, bool) -> Result<Vec<BalanceSheetRow>, BorsaError> + Send + Sync>,
    >,
    pub income_statement_fn: Option<
        Arc<dyn Fn(&Instrument, bool) -> Result<Vec<IncomeStatementRow>, BorsaError> + Send + Sync>,
    >,
    pub cashflow_fn: Option<
        Arc<dyn Fn(&Instrument, bool) -> Result<Vec<CashflowRow>, BorsaError> + Send + Sync>,
    >,
    pub analyst_price_target_fn:
        Option<Arc<dyn Fn(&Instrument) -> Result<PriceTarget, BorsaError> + Send + Sync>>,

    // Analysis providers
    pub recommendations_fn: Option<
        Arc<dyn Fn(&Instrument) -> Result<Vec<RecommendationRow>, BorsaError> + Send + Sync>,
    >,
    pub recommendations_summary_fn:
        Option<Arc<dyn Fn(&Instrument) -> Result<RecommendationSummary, BorsaError> + Send + Sync>>,
    pub upgrades_downgrades_fn: Option<
        Arc<dyn Fn(&Instrument) -> Result<Vec<UpgradeDowngradeRow>, BorsaError> + Send + Sync>,
    >,

    // Holders
    pub major_holders_fn:
        Option<Arc<dyn Fn(&Instrument) -> Result<Vec<MajorHolder>, BorsaError> + Send + Sync>>,

    // Options
    pub options_expirations_fn:
        Option<Arc<dyn Fn(&Instrument) -> Result<Vec<i64>, BorsaError> + Send + Sync>>,
    pub option_chain_fn: Option<
        Arc<dyn Fn(&Instrument, Option<i64>) -> Result<OptionChain, BorsaError> + Send + Sync>,
    >,

    // News
    pub news_fn: Option<
        Arc<dyn Fn(&Instrument, NewsRequest) -> Result<Vec<NewsArticle>, BorsaError> + Send + Sync>,
    >,

    // ESG
    pub esg_fn: Option<Arc<dyn Fn(&Instrument) -> Result<EsgScores, BorsaError> + Send + Sync>>,
}

impl Default for MockConnector {
    fn default() -> Self {
        Self {
            name: "default_mock",

            kind_ok: None,
            quote: None,
            history: None,
            search: None,
            delay_ms: 0,
            stream_updates: None,
            stream_start_error: None,
            history_intervals: DEFAULT_HISTORY_INTERVALS,
            stream_steps: None,

            quote_fn: None,
            history_fn: None,
            search_fn: None,
            calendar_fn: None,
            balance_sheet_fn: None,
            income_statement_fn: None,
            cashflow_fn: None,
            analyst_price_target_fn: None,

            recommendations_fn: None,
            recommendations_summary_fn: None,
            upgrades_downgrades_fn: None,

            major_holders_fn: None,

            options_expirations_fn: None,
            option_chain_fn: None,

            news_fn: None,
            esg_fn: None,
        }
    }
}

#[async_trait]
impl QuoteProvider for MockConnector {
    async fn quote(&self, i: &Instrument) -> Result<Quote, BorsaError> {
        if self.delay_ms > 0 {
            sleep(Duration::from_millis(self.delay_ms)).await;
        }

        if let Some(f) = &self.quote_fn {
            let mut q = (f)(i)?;
            q.symbol = i.symbol().clone();
            return Ok(q);
        }

        self.quote
            .clone()
            .map(|mut q| {
                q.symbol = i.symbol().clone();
                q
            })
            .ok_or_else(|| BorsaError::unsupported("quote"))
    }
}

#[async_trait]
impl HistoryProvider for MockConnector {
    async fn history(
        &self,
        _i: &Instrument,
        _r: HistoryRequest,
    ) -> Result<HistoryResponse, BorsaError> {
        if self.delay_ms > 0 {
            sleep(Duration::from_millis(self.delay_ms)).await;
        }

        if let Some(f) = &self.history_fn {
            return (f)(_i, _r);
        }

        self.history
            .clone()
            .ok_or_else(|| BorsaError::unsupported("history"))
    }

    fn supported_history_intervals(&self, _k: AssetKind) -> &'static [borsa_core::Interval] {
        self.history_intervals
    }
}

#[async_trait]
impl SearchProvider for MockConnector {
    async fn search(&self, _req: SearchRequest) -> Result<SearchResponse, BorsaError> {
        if let Some(f) = &self.search_fn {
            return (f)(_req);
        }

        self.search.clone().map_or_else(
            || Err(BorsaError::unsupported("search")),
            |v| Ok(SearchResponse { results: v }),
        )
    }
}

#[async_trait]
impl CalendarProvider for MockConnector {
    async fn calendar(&self, i: &Instrument) -> Result<Calendar, BorsaError> {
        if self.delay_ms > 0 {
            sleep(Duration::from_millis(self.delay_ms)).await;
        }
        if let Some(f) = &self.calendar_fn {
            return (f)(i);
        }
        Err(BorsaError::unsupported("calendar"))
    }
}

#[async_trait]
impl BalanceSheetProvider for MockConnector {
    async fn balance_sheet(
        &self,
        i: &Instrument,
        quarterly: bool,
    ) -> Result<Vec<BalanceSheetRow>, BorsaError> {
        if self.delay_ms > 0 {
            sleep(Duration::from_millis(self.delay_ms)).await;
        }
        if let Some(f) = &self.balance_sheet_fn {
            return (f)(i, quarterly);
        }
        Err(BorsaError::unsupported("balance_sheet"))
    }
}

#[async_trait]
impl IncomeStatementProvider for MockConnector {
    async fn income_statement(
        &self,
        i: &Instrument,
        quarterly: bool,
    ) -> Result<Vec<IncomeStatementRow>, BorsaError> {
        if self.delay_ms > 0 {
            sleep(Duration::from_millis(self.delay_ms)).await;
        }
        if let Some(f) = &self.income_statement_fn {
            return (f)(i, quarterly);
        }
        Err(BorsaError::unsupported("income_statement"))
    }
}

#[async_trait]
impl CashflowProvider for MockConnector {
    async fn cashflow(
        &self,
        i: &Instrument,
        quarterly: bool,
    ) -> Result<Vec<CashflowRow>, BorsaError> {
        if self.delay_ms > 0 {
            sleep(Duration::from_millis(self.delay_ms)).await;
        }
        if let Some(f) = &self.cashflow_fn {
            return (f)(i, quarterly);
        }
        Err(BorsaError::unsupported("cashflow"))
    }
}

#[async_trait]
impl AnalystPriceTargetProvider for MockConnector {
    async fn analyst_price_target(&self, i: &Instrument) -> Result<PriceTarget, BorsaError> {
        if self.delay_ms > 0 {
            sleep(Duration::from_millis(self.delay_ms)).await;
        }
        if let Some(f) = &self.analyst_price_target_fn {
            return (f)(i);
        }
        Err(BorsaError::unsupported("analyst_price_target"))
    }
}

#[derive(Clone)]
pub enum StreamStep {
    StartError(&'static str),
    Updates(Vec<borsa_core::QuoteUpdate>),
}

#[async_trait]
impl StreamProvider for MockConnector {
    async fn stream_quotes(
        &self,
        instruments: &[Instrument],
    ) -> Result<
        (
            borsa_core::stream::StreamHandle,
            tokio::sync::mpsc::Receiver<borsa_core::QuoteUpdate>,
        ),
        BorsaError,
    > {
        // Resolve scripted behavior first, falling back to static config
        let resolved = if let Some(steps) = &self.stream_steps {
            let mut guard = steps.lock().await;
            if guard.is_empty() {
                None
            } else {
                Some(guard.remove(0))
            }
        } else {
            None
        };

        let updates = match resolved {
            Some(StreamStep::StartError(msg)) => {
                return Err(BorsaError::Other(msg.to_string()));
            }
            Some(StreamStep::Updates(u)) => u,
            None => {
                if let Some(msg) = self.stream_start_error {
                    return Err(BorsaError::Other(msg.to_string()));
                }
                self.stream_updates
                    .clone()
                    .ok_or_else(|| BorsaError::unsupported("stream"))?
            }
        };

        let allow: std::collections::HashSet<String> =
            instruments.iter().map(|i| i.symbol().to_string()).collect();

        let (tx, rx) = tokio::sync::mpsc::channel::<borsa_core::QuoteUpdate>(1024);
        let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();
        let delay_ms = self.delay_ms;
        let join = tokio::spawn(async move {
            for u in updates {
                if !allow.is_empty() && !allow.contains(u.symbol.as_str()) {
                    continue;
                }
                tokio::select! {
                    biased;
                    _ = &mut stop_rx => {
                        return;
                    }
                    () = async {
                        if delay_ms > 0 {
                            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                        }
                    } => {
                        if tx.send(u).await.is_err() {
                            return;
                        }
                    }
                }
            }
            let _ = tokio::time::timeout(Duration::from_millis(50), &mut stop_rx).await;
        });

        Ok((borsa_core::stream::StreamHandle::new(join, stop_tx), rx))
    }
}

#[async_trait]
impl RecommendationsProvider for MockConnector {
    async fn recommendations(&self, i: &Instrument) -> Result<Vec<RecommendationRow>, BorsaError> {
        if self.delay_ms > 0 {
            sleep(Duration::from_millis(self.delay_ms)).await;
        }
        if let Some(f) = &self.recommendations_fn {
            return (f)(i);
        }
        Err(BorsaError::unsupported("analysis/recommendations"))
    }
}

#[async_trait]
impl RecommendationsSummaryProvider for MockConnector {
    async fn recommendations_summary(
        &self,
        i: &Instrument,
    ) -> Result<RecommendationSummary, BorsaError> {
        if self.delay_ms > 0 {
            sleep(Duration::from_millis(self.delay_ms)).await;
        }
        if let Some(f) = &self.recommendations_summary_fn {
            return (f)(i);
        }
        Err(BorsaError::unsupported("analysis/recommendations_summary"))
    }
}

#[async_trait]
impl UpgradesDowngradesProvider for MockConnector {
    async fn upgrades_downgrades(
        &self,
        i: &Instrument,
    ) -> Result<Vec<UpgradeDowngradeRow>, BorsaError> {
        if self.delay_ms > 0 {
            sleep(Duration::from_millis(self.delay_ms)).await;
        }
        if let Some(f) = &self.upgrades_downgrades_fn {
            return (f)(i);
        }
        Err(BorsaError::unsupported("analysis/upgrades_downgrades"))
    }
}

#[async_trait]
impl MajorHoldersProvider for MockConnector {
    async fn major_holders(&self, i: &Instrument) -> Result<Vec<MajorHolder>, BorsaError> {
        if self.delay_ms > 0 {
            sleep(Duration::from_millis(self.delay_ms)).await;
        }
        if let Some(f) = &self.major_holders_fn {
            return (f)(i);
        }
        Err(BorsaError::unsupported("major_holders"))
    }
}

#[async_trait]
impl OptionsExpirationsProvider for MockConnector {
    async fn options_expirations(&self, i: &Instrument) -> Result<Vec<i64>, BorsaError> {
        if self.delay_ms > 0 {
            sleep(Duration::from_millis(self.delay_ms)).await;
        }
        if let Some(f) = &self.options_expirations_fn {
            return (f)(i);
        }
        Err(BorsaError::unsupported("options/expirations"))
    }
}

#[async_trait]
impl OptionChainProvider for MockConnector {
    async fn option_chain(
        &self,
        i: &Instrument,
        date: Option<i64>,
    ) -> Result<OptionChain, BorsaError> {
        if self.delay_ms > 0 {
            sleep(Duration::from_millis(self.delay_ms)).await;
        }
        if let Some(f) = &self.option_chain_fn {
            return (f)(i, date);
        }
        Err(BorsaError::unsupported("options/chain"))
    }
}

#[async_trait]
impl NewsProvider for MockConnector {
    async fn news(&self, i: &Instrument, req: NewsRequest) -> Result<Vec<NewsArticle>, BorsaError> {
        if self.delay_ms > 0 {
            sleep(Duration::from_millis(self.delay_ms)).await;
        }
        if let Some(f) = &self.news_fn {
            return (f)(i, req);
        }
        Err(BorsaError::unsupported("news"))
    }
}

#[async_trait]
impl EsgProvider for MockConnector {
    async fn sustainability(&self, i: &Instrument) -> Result<EsgScores, BorsaError> {
        if self.delay_ms > 0 {
            sleep(Duration::from_millis(self.delay_ms)).await;
        }
        if let Some(f) = &self.esg_fn {
            return (f)(i);
        }
        Err(BorsaError::unsupported("esg"))
    }
}

#[async_trait]
impl BorsaConnector for MockConnector {
    fn name(&self) -> &'static str {
        self.name
    }

    fn supports_kind(&self, kind: AssetKind) -> bool {
        self.kind_ok.as_ref().is_none_or(|k| k == &kind)
    }

    fn as_history_provider(&self) -> Option<&dyn borsa_core::connector::HistoryProvider> {
        if self.history_fn.is_some() || self.history.is_some() {
            Some(self as &dyn HistoryProvider)
        } else {
            None
        }
    }

    fn as_quote_provider(&self) -> Option<&dyn borsa_core::connector::QuoteProvider> {
        if self.quote_fn.is_some() || self.quote.is_some() {
            Some(self as &dyn QuoteProvider)
        } else {
            None
        }
    }

    fn as_search_provider(&self) -> Option<&dyn borsa_core::connector::SearchProvider> {
        if self.search_fn.is_some() || self.search.is_some() {
            Some(self as &dyn SearchProvider)
        } else {
            None
        }
    }

    fn as_calendar_provider(&self) -> Option<&dyn borsa_core::connector::CalendarProvider> {
        if self.calendar_fn.is_some() {
            Some(self as &dyn CalendarProvider)
        } else {
            None
        }
    }

    fn as_balance_sheet_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::BalanceSheetProvider> {
        if self.balance_sheet_fn.is_some() {
            Some(self as &dyn BalanceSheetProvider)
        } else {
            None
        }
    }

    fn as_income_statement_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::IncomeStatementProvider> {
        if self.income_statement_fn.is_some() {
            Some(self as &dyn IncomeStatementProvider)
        } else {
            None
        }
    }

    fn as_cashflow_provider(&self) -> Option<&dyn borsa_core::connector::CashflowProvider> {
        if self.cashflow_fn.is_some() {
            Some(self as &dyn CashflowProvider)
        } else {
            None
        }
    }

    fn as_analyst_price_target_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::AnalystPriceTargetProvider> {
        if self.analyst_price_target_fn.is_some() {
            Some(self as &dyn AnalystPriceTargetProvider)
        } else {
            None
        }
    }

    fn as_stream_provider(&self) -> Option<&dyn borsa_core::connector::StreamProvider> {
        if self.stream_updates.is_some() || self.stream_start_error.is_some() {
            Some(self as &dyn StreamProvider)
        } else {
            None
        }
    }

    fn as_recommendations_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::RecommendationsProvider> {
        if self.recommendations_fn.is_some() {
            Some(self as &dyn RecommendationsProvider)
        } else {
            None
        }
    }

    fn as_recommendations_summary_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::RecommendationsSummaryProvider> {
        if self.recommendations_summary_fn.is_some() {
            Some(self as &dyn RecommendationsSummaryProvider)
        } else {
            None
        }
    }

    fn as_upgrades_downgrades_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::UpgradesDowngradesProvider> {
        if self.upgrades_downgrades_fn.is_some() {
            Some(self as &dyn UpgradesDowngradesProvider)
        } else {
            None
        }
    }

    fn as_major_holders_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::MajorHoldersProvider> {
        if self.major_holders_fn.is_some() {
            Some(self as &dyn MajorHoldersProvider)
        } else {
            None
        }
    }

    fn as_options_expirations_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::OptionsExpirationsProvider> {
        if self.options_expirations_fn.is_some() {
            Some(self as &dyn OptionsExpirationsProvider)
        } else {
            None
        }
    }

    fn as_option_chain_provider(&self) -> Option<&dyn borsa_core::connector::OptionChainProvider> {
        if self.option_chain_fn.is_some() {
            Some(self as &dyn OptionChainProvider)
        } else {
            None
        }
    }

    fn as_news_provider(&self) -> Option<&dyn borsa_core::connector::NewsProvider> {
        if self.news_fn.is_some() {
            Some(self as &dyn NewsProvider)
        } else {
            None
        }
    }

    fn as_esg_provider(&self) -> Option<&dyn borsa_core::connector::EsgProvider> {
        if self.esg_fn.is_some() {
            Some(self as &dyn EsgProvider)
        } else {
            None
        }
    }
}

/* ---------- Tiny builder helpers used by tests ---------- */

impl MockConnector {
    #[allow(dead_code)]
    pub fn builder() -> MockConnectorBuilder {
        MockConnectorBuilder::new()
    }
}

pub struct MockConnectorBuilder {
    name: &'static str,
    kind_ok: Option<AssetKind>,
    delay_ms: u64,
    history_intervals: &'static [borsa_core::Interval],
    quote_fn: Option<Arc<dyn Fn(&Instrument) -> Result<Quote, BorsaError> + Send + Sync>>,
    history_fn: Option<
        Arc<
            dyn Fn(&Instrument, HistoryRequest) -> Result<HistoryResponse, BorsaError>
                + Send
                + Sync,
        >,
    >,
    search_fn:
        Option<Arc<dyn Fn(SearchRequest) -> Result<SearchResponse, BorsaError> + Send + Sync>>,
    calendar_fn: Option<Arc<dyn Fn(&Instrument) -> Result<Calendar, BorsaError> + Send + Sync>>,
    balance_sheet_fn: Option<
        Arc<dyn Fn(&Instrument, bool) -> Result<Vec<BalanceSheetRow>, BorsaError> + Send + Sync>,
    >,
    income_statement_fn: Option<
        Arc<dyn Fn(&Instrument, bool) -> Result<Vec<IncomeStatementRow>, BorsaError> + Send + Sync>,
    >,
    cashflow_fn: Option<
        Arc<dyn Fn(&Instrument, bool) -> Result<Vec<CashflowRow>, BorsaError> + Send + Sync>,
    >,
    analyst_price_target_fn:
        Option<Arc<dyn Fn(&Instrument) -> Result<PriceTarget, BorsaError> + Send + Sync>>,
    stream_updates: Option<Vec<borsa_core::QuoteUpdate>>,
    stream_start_error: Option<&'static str>,
    stream_steps: Option<Arc<Mutex<Vec<StreamStep>>>>,

    // Analysis
    recommendations_fn: Option<
        Arc<dyn Fn(&Instrument) -> Result<Vec<RecommendationRow>, BorsaError> + Send + Sync>,
    >,
    recommendations_summary_fn:
        Option<Arc<dyn Fn(&Instrument) -> Result<RecommendationSummary, BorsaError> + Send + Sync>>,
    upgrades_downgrades_fn: Option<
        Arc<dyn Fn(&Instrument) -> Result<Vec<UpgradeDowngradeRow>, BorsaError> + Send + Sync>,
    >,

    // Holders
    major_holders_fn:
        Option<Arc<dyn Fn(&Instrument) -> Result<Vec<MajorHolder>, BorsaError> + Send + Sync>>,

    // Options
    options_expirations_fn:
        Option<Arc<dyn Fn(&Instrument) -> Result<Vec<i64>, BorsaError> + Send + Sync>>,
    option_chain_fn: Option<
        Arc<dyn Fn(&Instrument, Option<i64>) -> Result<OptionChain, BorsaError> + Send + Sync>,
    >,

    // News
    news_fn: Option<
        Arc<dyn Fn(&Instrument, NewsRequest) -> Result<Vec<NewsArticle>, BorsaError> + Send + Sync>,
    >,
    esg_fn: Option<Arc<dyn Fn(&Instrument) -> Result<EsgScores, BorsaError> + Send + Sync>>,
}

impl MockConnectorBuilder {
    pub fn new() -> Self {
        Self {
            name: "mock",
            kind_ok: None,
            delay_ms: 0,
            history_intervals: DEFAULT_HISTORY_INTERVALS,
            quote_fn: None,
            history_fn: None,
            search_fn: None,
            calendar_fn: None,
            balance_sheet_fn: None,
            income_statement_fn: None,
            cashflow_fn: None,
            analyst_price_target_fn: None,
            stream_updates: None,
            stream_start_error: None,
            stream_steps: None,

            recommendations_fn: None,
            recommendations_summary_fn: None,
            upgrades_downgrades_fn: None,

            major_holders_fn: None,

            options_expirations_fn: None,
            option_chain_fn: None,

            news_fn: None,
            esg_fn: None,
        }
    }

    pub fn name(mut self, name: &'static str) -> Self {
        self.name = name;
        self
    }
    pub fn supports_kind(mut self, kind: AssetKind) -> Self {
        self.kind_ok = Some(kind);
        self
    }
    pub fn delay(mut self, d: Duration) -> Self {
        self.delay_ms = d.as_millis() as u64;
        self
    }
    pub fn with_history_intervals(mut self, intervals: &'static [borsa_core::Interval]) -> Self {
        self.history_intervals = intervals;
        self
    }
    pub fn with_stream_updates(mut self, updates: Vec<borsa_core::QuoteUpdate>) -> Self {
        self.stream_updates = Some(updates);
        self
    }
    pub fn with_stream_steps(mut self, steps: Vec<StreamStep>) -> Self {
        self.stream_steps = Some(Arc::new(Mutex::new(steps)));
        self
    }
    pub fn will_fail_stream_start(mut self, msg: &'static str) -> Self {
        self.stream_start_error = Some(msg);
        self
    }

    // Quote
    pub fn with_quote_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(&Instrument) -> Result<Quote, BorsaError> + Send + Sync + 'static,
    {
        self.quote_fn = Some(Arc::new(f));
        self
    }
    pub fn returns_quote_ok(mut self, q: Quote) -> Self {
        self.quote_fn = Some(Arc::new(move |_i| Ok(q.clone())));
        self
    }

    // History
    pub fn with_history_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(&Instrument, HistoryRequest) -> Result<HistoryResponse, BorsaError>
            + Send
            + Sync
            + 'static,
    {
        self.history_fn = Some(Arc::new(f));
        self
    }
    pub fn returns_history_ok(mut self, resp: HistoryResponse) -> Self {
        self.history_fn = Some(Arc::new(move |_i, _r| Ok(resp.clone())));
        self
    }
    pub fn history_with_delay_ok(mut self, d: Duration, resp: HistoryResponse) -> Self {
        self.delay_ms = d.as_millis() as u64;
        self.history_fn = Some(Arc::new(move |_i, _r| Ok(resp.clone())));
        self
    }

    // Search
    pub fn with_search_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(SearchRequest) -> Result<SearchResponse, BorsaError> + Send + Sync + 'static,
    {
        self.search_fn = Some(Arc::new(f));
        self
    }
    pub fn returns_search_ok(mut self, results: Vec<SearchResult>) -> Self {
        self.search_fn = Some(Arc::new(move |_req| {
            Ok(SearchResponse {
                results: results.clone(),
            })
        }));
        self
    }

    // Calendar
    pub fn with_calendar_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(&Instrument) -> Result<Calendar, BorsaError> + Send + Sync + 'static,
    {
        self.calendar_fn = Some(Arc::new(f));
        self
    }
    pub fn returns_calendar_ok(mut self, cal: Calendar) -> Self {
        self.calendar_fn = Some(Arc::new(move |_i| Ok(cal.clone())));
        self
    }

    // Balance sheet
    pub fn with_balance_sheet_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(&Instrument, bool) -> Result<Vec<BalanceSheetRow>, BorsaError>
            + Send
            + Sync
            + 'static,
    {
        self.balance_sheet_fn = Some(Arc::new(f));
        self
    }
    pub fn returns_balance_sheet_ok(mut self, rows: Vec<BalanceSheetRow>) -> Self {
        self.balance_sheet_fn = Some(Arc::new(move |_i, _q| Ok(rows.clone())));
        self
    }

    // Income statement
    pub fn with_income_statement_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(&Instrument, bool) -> Result<Vec<IncomeStatementRow>, BorsaError>
            + Send
            + Sync
            + 'static,
    {
        self.income_statement_fn = Some(Arc::new(f));
        self
    }
    pub fn returns_income_statement_ok(mut self, rows: Vec<IncomeStatementRow>) -> Self {
        self.income_statement_fn = Some(Arc::new(move |_i, _q| Ok(rows.clone())));
        self
    }

    // Cashflow
    pub fn with_cashflow_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(&Instrument, bool) -> Result<Vec<CashflowRow>, BorsaError> + Send + Sync + 'static,
    {
        self.cashflow_fn = Some(Arc::new(f));
        self
    }
    pub fn returns_cashflow_ok(mut self, rows: Vec<CashflowRow>) -> Self {
        self.cashflow_fn = Some(Arc::new(move |_i, _q| Ok(rows.clone())));
        self
    }

    // Analyst price target
    pub fn with_analyst_price_target_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(&Instrument) -> Result<PriceTarget, BorsaError> + Send + Sync + 'static,
    {
        self.analyst_price_target_fn = Some(Arc::new(f));
        self
    }
    pub fn returns_analyst_price_target_ok(mut self, pt: PriceTarget) -> Self {
        self.analyst_price_target_fn = Some(Arc::new(move |_i| Ok(pt.clone())));
        self
    }

    // Recommendations
    pub fn with_recommendations_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(&Instrument) -> Result<Vec<RecommendationRow>, BorsaError> + Send + Sync + 'static,
    {
        self.recommendations_fn = Some(Arc::new(f));
        self
    }
    pub fn returns_recommendations_ok(mut self, rows: Vec<RecommendationRow>) -> Self {
        self.recommendations_fn = Some(Arc::new(move |_i| Ok(rows.clone())));
        self
    }

    pub fn with_recommendations_summary_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(&Instrument) -> Result<RecommendationSummary, BorsaError> + Send + Sync + 'static,
    {
        self.recommendations_summary_fn = Some(Arc::new(f));
        self
    }
    pub fn returns_recommendations_summary_ok(mut self, sum: RecommendationSummary) -> Self {
        self.recommendations_summary_fn = Some(Arc::new(move |_i| Ok(sum.clone())));
        self
    }

    pub fn with_upgrades_downgrades_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(&Instrument) -> Result<Vec<UpgradeDowngradeRow>, BorsaError> + Send + Sync + 'static,
    {
        self.upgrades_downgrades_fn = Some(Arc::new(f));
        self
    }
    pub fn returns_upgrades_downgrades_ok(mut self, rows: Vec<UpgradeDowngradeRow>) -> Self {
        self.upgrades_downgrades_fn = Some(Arc::new(move |_i| Ok(rows.clone())));
        self
    }

    // Major holders
    pub fn with_major_holders_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(&Instrument) -> Result<Vec<MajorHolder>, BorsaError> + Send + Sync + 'static,
    {
        self.major_holders_fn = Some(Arc::new(f));
        self
    }
    pub fn returns_major_holders_ok(mut self, rows: Vec<MajorHolder>) -> Self {
        self.major_holders_fn = Some(Arc::new(move |_i| Ok(rows.clone())));
        self
    }

    // Options
    pub fn with_options_expirations_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(&Instrument) -> Result<Vec<i64>, BorsaError> + Send + Sync + 'static,
    {
        self.options_expirations_fn = Some(Arc::new(f));
        self
    }
    pub fn returns_options_expirations_ok(mut self, dates: Vec<i64>) -> Self {
        self.options_expirations_fn = Some(Arc::new(move |_i| Ok(dates.clone())));
        self
    }

    pub fn with_option_chain_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(&Instrument, Option<i64>) -> Result<OptionChain, BorsaError> + Send + Sync + 'static,
    {
        self.option_chain_fn = Some(Arc::new(f));
        self
    }
    pub fn returns_option_chain_ok(mut self, chain: OptionChain) -> Self {
        self.option_chain_fn = Some(Arc::new(move |_i, _d| Ok(chain.clone())));
        self
    }

    // News
    pub fn with_news_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(&Instrument, NewsRequest) -> Result<Vec<NewsArticle>, BorsaError>
            + Send
            + Sync
            + 'static,
    {
        self.news_fn = Some(Arc::new(f));
        self
    }
    pub fn returns_news_ok(mut self, articles: Vec<NewsArticle>) -> Self {
        self.news_fn = Some(Arc::new(move |_i, _r| Ok(articles.clone())));
        self
    }

    // ESG
    pub fn with_esg_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(&Instrument) -> Result<EsgScores, BorsaError> + Send + Sync + 'static,
    {
        self.esg_fn = Some(Arc::new(f));
        self
    }
    pub fn returns_esg_ok(mut self, scores: EsgScores) -> Self {
        self.esg_fn = Some(Arc::new(move |_i| Ok(scores.clone())));
        self
    }

    pub fn build(self) -> Arc<MockConnector> {
        Arc::new(MockConnector {
            name: self.name,
            kind_ok: self.kind_ok,
            quote: None,
            history: None,
            search: None,
            delay_ms: self.delay_ms,
            stream_updates: self.stream_updates,
            stream_start_error: self.stream_start_error,
            history_intervals: self.history_intervals,
            stream_steps: self.stream_steps,
            quote_fn: self.quote_fn,
            history_fn: self.history_fn,
            search_fn: self.search_fn,
            calendar_fn: self.calendar_fn,
            balance_sheet_fn: self.balance_sheet_fn,
            income_statement_fn: self.income_statement_fn,
            cashflow_fn: self.cashflow_fn,
            analyst_price_target_fn: self.analyst_price_target_fn,

            recommendations_fn: self.recommendations_fn,
            recommendations_summary_fn: self.recommendations_summary_fn,
            upgrades_downgrades_fn: self.upgrades_downgrades_fn,

            major_holders_fn: self.major_holders_fn,

            options_expirations_fn: self.options_expirations_fn,
            option_chain_fn: self.option_chain_fn,

            news_fn: self.news_fn,
            esg_fn: self.esg_fn,
        })
    }
}

/// Convenience constructor for a quote-only mock connector.
#[allow(dead_code)]
pub fn m_quote(name: &'static str, last: f64) -> Arc<MockConnector> {
    MockConnector::builder()
        .name(name)
        .returns_quote_ok(Quote {
            symbol: borsa_core::Symbol::new("X").unwrap(),
            shortname: None,
            price: Some(crate::helpers::usd(&last.to_string())),
            previous_close: None,
            exchange: None,
            market_state: None,
            day_volume: None,
        })
        .build()
}

/// Convenience constructor for a history-only mock connector.
#[allow(dead_code)]
pub fn m_hist(name: &'static str, ts: &[i64]) -> Arc<MockConnector> {
    let candles = ts
        .iter()
        .copied()
        .map(|t| {
            let v: f64 = f64::from(i32::try_from(t).unwrap());
            candle(t, v)
        })
        .collect::<Vec<_>>();

    MockConnector::builder()
        .name(name)
        .returns_history_ok(HistoryResponse {
            candles,
            actions: vec![],
            adjusted: false,
            meta: None,
        })
        .build()
}

/// Convenience constructor for a search-only mock connector.
#[allow(dead_code)]
pub fn m_search(name: &'static str, items: Vec<SearchResult>) -> Arc<MockConnector> {
    MockConnector::builder()
        .name(name)
        .returns_search_ok(items)
        .build()
}

/// Create a candle with all OHLC equal to `close` (handy in tests).
#[allow(dead_code)]
pub fn candle(ts: i64, close: f64) -> Candle {
    use chrono::TimeZone;
    let ts = chrono::Utc.timestamp_opt(ts, 0).unwrap();
    let price = crate::helpers::usd(&close.to_string());
    Candle {
        ts,
        open: price.clone(),
        high: price.clone(),
        low: price.clone(),
        close: price,
        close_unadj: None,
        volume: None,
    }
}
