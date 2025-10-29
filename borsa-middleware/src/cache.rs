use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use borsa_core::Exchange;
use borsa_core::connector::{
    AnalystPriceTargetProvider, BalanceSheetProvider, CalendarProvider, CashflowProvider,
    EarningsProvider, EsgProvider, HistoryProvider, IncomeStatementProvider,
    InsiderRosterHoldersProvider, InsiderTransactionsProvider, InstitutionalHoldersProvider,
    IsinProvider, MajorHoldersProvider, MutualFundHoldersProvider,
    NetSharePurchaseActivityProvider, NewsProvider, OptionChainProvider,
    OptionsExpirationsProvider, ProfileProvider, QuoteProvider, RecommendationsProvider,
    RecommendationsSummaryProvider, SearchProvider, StreamProvider, UpgradesDowngradesProvider,
};
use borsa_core::{
    AssetKind, BalanceSheetRow, BorsaConnector, BorsaError, Calendar, CashflowRow, Earnings,
    EsgScores, HistoryRequest, HistoryResponse, IncomeStatementRow, Instrument, Interval, Isin,
    NewsArticle, NewsRequest, NewsTab, OptionChain, PriceTarget, Profile, Quote, Range,
    RecommendationRow, RecommendationSummary, SearchRequest, SearchResponse, UpgradeDowngradeRow,
};
use borsa_types::{CacheConfig, Capability};
use lru::LruCache;
use tokio::sync::Mutex;

/// Small helper: identity of an instrument for caching discrimination.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct InstrumentKey {
    symbol: String,
    kind: AssetKind,
    exchange: Option<Exchange>,
}

impl From<&Instrument> for InstrumentKey {
    fn from(i: &Instrument) -> Self {
        Self {
            symbol: i.symbol().to_string(),
            kind: *i.kind(),
            exchange: i.exchange().cloned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct HistoryKey {
    inst: InstrumentKey,
    interval: IntervalKey,
    range: RangeKey,
    flags: u8,
}

impl HistoryKey {
    const INCLUDE_PREPOST: u8 = 1 << 0;
    const INCLUDE_ACTIONS: u8 = 1 << 1;
    const AUTO_ADJUST: u8 = 1 << 2;
    const KEEPNA: u8 = 1 << 3;

    fn from_request(inst: &Instrument, req: &HistoryRequest) -> Self {
        let mut flags = 0u8;
        if req.include_prepost() {
            flags |= Self::INCLUDE_PREPOST;
        }
        if req.include_actions() {
            flags |= Self::INCLUDE_ACTIONS;
        }
        if req.auto_adjust() {
            flags |= Self::AUTO_ADJUST;
        }
        if req.keepna() {
            flags |= Self::KEEPNA;
        }
        Self {
            inst: InstrumentKey::from(inst),
            interval: IntervalKey(req.interval()),
            range: RangeKey(req.range()),
            flags,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct BoolByInstrumentKey {
    inst: InstrumentKey,
    flag: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct OptionChainKey {
    inst: InstrumentKey,
    date: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SearchKey {
    query: String,
    kind: Option<AssetKind>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct NewsKey {
    inst: InstrumentKey,
    count: u32,
    tab: NewsTabKey,
}

#[derive(Clone, Copy)]
struct IntervalKey(Interval);

impl std::fmt::Debug for IntervalKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IntervalKey(..)")
    }
}

impl PartialEq for IntervalKey {
    fn eq(&self, other: &Self) -> bool {
        std::mem::discriminant(&self.0) == std::mem::discriminant(&other.0)
    }
}
impl Eq for IntervalKey {}
impl std::hash::Hash for IntervalKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(&self.0).hash(state);
    }
}

#[derive(Clone, Copy)]
struct RangeKey(Option<Range>);

impl std::fmt::Debug for RangeKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RangeKey(..)")
    }
}
impl PartialEq for RangeKey {
    fn eq(&self, other: &Self) -> bool {
        match (self.0, other.0) {
            (None, None) => true,
            (Some(a), Some(b)) => std::mem::discriminant(&a) == std::mem::discriminant(&b),
            _ => false,
        }
    }
}
impl Eq for RangeKey {}
impl std::hash::Hash for RangeKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self.0 {
            None => 0u8.hash(state),
            Some(ref r) => {
                1u8.hash(state);
                std::mem::discriminant(r).hash(state);
            }
        }
    }
}

#[derive(Clone, Copy)]
struct NewsTabKey(NewsTab);

impl std::fmt::Debug for NewsTabKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NewsTabKey(..)")
    }
}
impl PartialEq for NewsTabKey {
    fn eq(&self, other: &Self) -> bool {
        std::mem::discriminant(&self.0) == std::mem::discriminant(&other.0)
    }
}
impl Eq for NewsTabKey {}
impl std::hash::Hash for NewsTabKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(&self.0).hash(state);
    }
}

#[async_trait]
trait CacheStore<K, V>: Send + Sync {
    async fn get(&self, key: &K) -> Option<V>;
    async fn put(&self, key: K, value: V);
}

struct Entry<V> {
    value: V,
    expires_at: std::time::Instant,
}

struct LruTtlStore<K, V> {
    inner: Mutex<LruCache<K, Entry<V>>>,
    ttl: Duration,
}

impl<K, V> LruTtlStore<K, V>
where
    K: std::hash::Hash + Eq,
{
    fn new(capacity: usize, ttl: Duration) -> Self {
        // Avoid zero capacity panics
        let cap = capacity.max(1);
        let cap_nz = std::num::NonZeroUsize::new(cap).unwrap();
        Self {
            inner: Mutex::new(LruCache::new(cap_nz)),
            ttl,
        }
    }
}

#[async_trait]
impl<K, V> CacheStore<K, V> for LruTtlStore<K, V>
where
    K: Clone + std::hash::Hash + Eq + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    async fn get(&self, key: &K) -> Option<V> {
        let mut guard = self.inner.lock().await;
        if let Some(entry) = guard.get_mut(key)
            && std::time::Instant::now() <= entry.expires_at
        {
            return Some(entry.value.clone());
        }
        // If expired, remove it and return None
        guard.pop(key).and_then(|_| None)
    }
    async fn put(&self, key: K, value: V) {
        let expires_at = std::time::Instant::now() + self.ttl;
        let mut guard = self.inner.lock().await;
        guard.put(key, Entry { value, expires_at });
    }
}

/// Declarative wrapper that applies caching when building a connector stack.
pub struct CacheMiddleware {
    cfg: CacheConfig,
}

impl CacheMiddleware {
    #[must_use]
    pub const fn new(cfg: CacheConfig) -> Self {
        Self { cfg }
    }
}

impl borsa_core::Middleware for CacheMiddleware {
    fn apply(self: Box<Self>, inner: Arc<dyn BorsaConnector>) -> Arc<dyn BorsaConnector> {
        let Self { cfg } = *self;
        Arc::new(CachingConnector::new(inner, &cfg))
    }

    fn name(&self) -> &'static str {
        "CachingMiddleware"
    }

    fn config_json(&self) -> serde_json::Value {
        serde_json::json!({
            "default_ttl_ms": self.cfg.default_ttl_ms,
            "default_max_entries": self.cfg.default_max_entries,
            "per_capability_ttl_ms": self.cfg.per_capability_ttl_ms,
            "per_capability_max_entries": self.cfg.per_capability_max_entries,
        })
    }
}

// Per-capability typed stores; `None` means disabled (e.g., TTL=0).
struct Stores {
    quote: Option<Arc<dyn CacheStore<InstrumentKey, Arc<Quote>>>>,
    profile: Option<Arc<dyn CacheStore<InstrumentKey, Arc<Profile>>>>,
    isin: Option<Arc<dyn CacheStore<InstrumentKey, Arc<Option<Isin>>>>>,
    history: Option<Arc<dyn CacheStore<HistoryKey, Arc<HistoryResponse>>>>,
    earnings: Option<Arc<dyn CacheStore<InstrumentKey, Arc<Earnings>>>>,
    income_stmt: Option<Arc<dyn CacheStore<BoolByInstrumentKey, Arc<Vec<IncomeStatementRow>>>>>,
    balance_sheet: Option<Arc<dyn CacheStore<BoolByInstrumentKey, Arc<Vec<BalanceSheetRow>>>>>,
    cashflow: Option<Arc<dyn CacheStore<BoolByInstrumentKey, Arc<Vec<CashflowRow>>>>>,
    calendar: Option<Arc<dyn CacheStore<InstrumentKey, Arc<Calendar>>>>,
    recommendations: Option<Arc<dyn CacheStore<InstrumentKey, Arc<Vec<RecommendationRow>>>>>,
    recommendations_summary: Option<Arc<dyn CacheStore<InstrumentKey, Arc<RecommendationSummary>>>>,
    upgrades_downgrades: Option<Arc<dyn CacheStore<InstrumentKey, Arc<Vec<UpgradeDowngradeRow>>>>>,
    analyst_price_target: Option<Arc<dyn CacheStore<InstrumentKey, Arc<PriceTarget>>>>,
    major_holders: Option<Arc<dyn CacheStore<InstrumentKey, Arc<Vec<borsa_core::MajorHolder>>>>>,
    institutional_holders:
        Option<Arc<dyn CacheStore<InstrumentKey, Arc<Vec<borsa_core::InstitutionalHolder>>>>>,
    mutual_fund_holders:
        Option<Arc<dyn CacheStore<InstrumentKey, Arc<Vec<borsa_core::InstitutionalHolder>>>>>,
    insider_transactions:
        Option<Arc<dyn CacheStore<InstrumentKey, Arc<Vec<borsa_core::InsiderTransaction>>>>>,
    insider_roster:
        Option<Arc<dyn CacheStore<InstrumentKey, Arc<Vec<borsa_core::InsiderRosterHolder>>>>>,
    net_share_purchase_activity: Option<
        Arc<dyn CacheStore<InstrumentKey, Arc<Option<borsa_core::NetSharePurchaseActivity>>>>,
    >,
    esg: Option<Arc<dyn CacheStore<InstrumentKey, Arc<EsgScores>>>>,
    news: Option<Arc<dyn CacheStore<NewsKey, Arc<Vec<NewsArticle>>>>>,
    options_expirations: Option<Arc<dyn CacheStore<InstrumentKey, Arc<Vec<i64>>>>>,
    option_chain: Option<Arc<dyn CacheStore<OptionChainKey, Arc<OptionChain>>>>,
    search: Option<Arc<dyn CacheStore<SearchKey, Arc<SearchResponse>>>>,
}

pub struct CachingConnector {
    inner: Arc<dyn BorsaConnector>,
    stores: Stores,
}

impl CachingConnector {
    fn maybe_store<K, V>(cfg: &CacheConfig, cap: Capability) -> Option<Arc<dyn CacheStore<K, V>>>
    where
        K: Clone + std::hash::Hash + Eq + Send + Sync + 'static,
        V: Clone + Send + Sync + 'static,
    {
        let ttl = cfg.ttl_for(cap)?;
        let capacity = cfg.capacity_for(cap);
        let store = LruTtlStore::<K, V>::new(capacity, ttl);
        Some(Arc::new(store))
    }

    #[must_use]
    pub fn new(inner: Arc<dyn BorsaConnector>, cfg: &CacheConfig) -> Self {
        let stores = Stores {
            quote: Self::maybe_store(cfg, Capability::Quote),
            profile: Self::maybe_store(cfg, Capability::Profile),
            isin: Self::maybe_store(cfg, Capability::Isin),
            history: Self::maybe_store(cfg, Capability::History),
            earnings: Self::maybe_store(cfg, Capability::Earnings),
            income_stmt: Self::maybe_store(cfg, Capability::IncomeStatement),
            balance_sheet: Self::maybe_store(cfg, Capability::BalanceSheet),
            cashflow: Self::maybe_store(cfg, Capability::Cashflow),
            calendar: Self::maybe_store(cfg, Capability::Calendar),
            recommendations: Self::maybe_store(cfg, Capability::Recommendations),
            recommendations_summary: Self::maybe_store(cfg, Capability::RecommendationsSummary),
            upgrades_downgrades: Self::maybe_store(cfg, Capability::UpgradesDowngrades),
            analyst_price_target: Self::maybe_store(cfg, Capability::AnalystPriceTarget),
            major_holders: Self::maybe_store(cfg, Capability::MajorHolders),
            institutional_holders: Self::maybe_store(cfg, Capability::InstitutionalHolders),
            mutual_fund_holders: Self::maybe_store(cfg, Capability::MutualFundHolders),
            insider_transactions: Self::maybe_store(cfg, Capability::InsiderTransactions),
            insider_roster: Self::maybe_store(cfg, Capability::InsiderRoster),
            net_share_purchase_activity: Self::maybe_store(
                cfg,
                Capability::NetSharePurchaseActivity,
            ),
            esg: Self::maybe_store(cfg, Capability::Esg),
            news: Self::maybe_store(cfg, Capability::News),
            options_expirations: Self::maybe_store(cfg, Capability::OptionsExpirations),
            option_chain: Self::maybe_store(cfg, Capability::OptionChain),
            search: Self::maybe_store(cfg, Capability::Search),
        };
        Self { inner, stores }
    }
}

#[borsa_macros::delegate_connector(inner)]
impl CachingConnector {}

#[async_trait]
impl borsa_core::Middleware for CachingConnector {
    fn apply(self: Box<Self>, _inner: Arc<dyn BorsaConnector>) -> Arc<dyn BorsaConnector> {
        unreachable!("CachingConnector is already applied")
    }
    fn name(&self) -> &'static str {
        "CachingMiddleware"
    }
    fn config_json(&self) -> serde_json::Value {
        serde_json::json!({})
    }
}

#[async_trait]
impl QuoteProvider for CachingConnector {
    async fn quote(&self, instrument: &Instrument) -> Result<Quote, BorsaError> {
        if let Some(store) = &self.stores.quote {
            let key = InstrumentKey::from(instrument);
            if let Some(v) = store.get(&key).await {
                return Ok((*v).clone());
            }
            let inner = self
                .inner
                .as_quote_provider()
                .ok_or_else(|| BorsaError::unsupported("quote"))?;
            let value = inner.quote(instrument).await?;
            store.put(key, Arc::new(value.clone())).await;
            return Ok(value);
        }
        self.inner
            .as_quote_provider()
            .ok_or_else(|| BorsaError::unsupported("quote"))?
            .quote(instrument)
            .await
    }
}

#[async_trait]
impl ProfileProvider for CachingConnector {
    async fn profile(&self, instrument: &Instrument) -> Result<Profile, BorsaError> {
        if let Some(store) = &self.stores.profile {
            let key = InstrumentKey::from(instrument);
            if let Some(v) = store.get(&key).await {
                return Ok((*v).clone());
            }
            let inner = self
                .inner
                .as_profile_provider()
                .ok_or_else(|| BorsaError::unsupported("profile"))?;
            let value = inner.profile(instrument).await?;
            store.put(key, Arc::new(value.clone())).await;
            return Ok(value);
        }
        self.inner
            .as_profile_provider()
            .ok_or_else(|| BorsaError::unsupported("profile"))?
            .profile(instrument)
            .await
    }
}

#[async_trait]
impl IsinProvider for CachingConnector {
    async fn isin(&self, instrument: &Instrument) -> Result<Option<Isin>, BorsaError> {
        if let Some(store) = &self.stores.isin {
            let key = InstrumentKey::from(instrument);
            if let Some(v) = store.get(&key).await {
                return Ok((*v).clone());
            }
            let inner = self
                .inner
                .as_isin_provider()
                .ok_or_else(|| BorsaError::unsupported("isin"))?;
            let value = inner.isin(instrument).await?;
            store.put(key, Arc::new(value.clone())).await;
            return Ok(value);
        }
        self.inner
            .as_isin_provider()
            .ok_or_else(|| BorsaError::unsupported("isin"))?
            .isin(instrument)
            .await
    }
}

#[async_trait]
impl HistoryProvider for CachingConnector {
    async fn history(
        &self,
        instrument: &Instrument,
        req: HistoryRequest,
    ) -> Result<HistoryResponse, BorsaError> {
        if let Some(store) = &self.stores.history {
            let key = HistoryKey::from_request(instrument, &req);
            if let Some(v) = store.get(&key).await {
                return Ok((*v).clone());
            }
            let inner = self
                .inner
                .as_history_provider()
                .ok_or_else(|| BorsaError::unsupported("history"))?;
            let value = inner.history(instrument, req).await?;
            store.put(key, Arc::new(value.clone())).await;
            return Ok(value);
        }
        self.inner
            .as_history_provider()
            .ok_or_else(|| BorsaError::unsupported("history"))?
            .history(instrument, req)
            .await
    }

    fn supported_history_intervals(&self, kind: AssetKind) -> &'static [Interval] {
        if let Some(inner) = self.inner.as_history_provider() {
            inner.supported_history_intervals(kind)
        } else {
            &[]
        }
    }
}

#[async_trait]
impl EarningsProvider for CachingConnector {
    async fn earnings(&self, instrument: &Instrument) -> Result<Earnings, BorsaError> {
        if let Some(store) = &self.stores.earnings {
            let key = InstrumentKey::from(instrument);
            if let Some(v) = store.get(&key).await {
                return Ok((*v).clone());
            }
            let inner = self
                .inner
                .as_earnings_provider()
                .ok_or_else(|| BorsaError::unsupported("earnings"))?;
            let value = inner.earnings(instrument).await?;
            store.put(key, Arc::new(value.clone())).await;
            return Ok(value);
        }
        self.inner
            .as_earnings_provider()
            .ok_or_else(|| BorsaError::unsupported("earnings"))?
            .earnings(instrument)
            .await
    }
}

#[async_trait]
impl IncomeStatementProvider for CachingConnector {
    async fn income_statement(
        &self,
        instrument: &Instrument,
        quarterly: bool,
    ) -> Result<Vec<IncomeStatementRow>, BorsaError> {
        if let Some(store) = &self.stores.income_stmt {
            let key = BoolByInstrumentKey {
                inst: InstrumentKey::from(instrument),
                flag: quarterly,
            };
            if let Some(v) = store.get(&key).await {
                return Ok((*v).clone());
            }
            let inner = self
                .inner
                .as_income_statement_provider()
                .ok_or_else(|| BorsaError::unsupported("income_statement"))?;
            let value = inner.income_statement(instrument, quarterly).await?;
            store.put(key, Arc::new(value.clone())).await;
            return Ok(value);
        }
        self.inner
            .as_income_statement_provider()
            .ok_or_else(|| BorsaError::unsupported("income_statement"))?
            .income_statement(instrument, quarterly)
            .await
    }
}

#[async_trait]
impl BalanceSheetProvider for CachingConnector {
    async fn balance_sheet(
        &self,
        instrument: &Instrument,
        quarterly: bool,
    ) -> Result<Vec<BalanceSheetRow>, BorsaError> {
        if let Some(store) = &self.stores.balance_sheet {
            let key = BoolByInstrumentKey {
                inst: InstrumentKey::from(instrument),
                flag: quarterly,
            };
            if let Some(v) = store.get(&key).await {
                return Ok((*v).clone());
            }
            let inner = self
                .inner
                .as_balance_sheet_provider()
                .ok_or_else(|| BorsaError::unsupported("balance_sheet"))?;
            let value = inner.balance_sheet(instrument, quarterly).await?;
            store.put(key, Arc::new(value.clone())).await;
            return Ok(value);
        }
        self.inner
            .as_balance_sheet_provider()
            .ok_or_else(|| BorsaError::unsupported("balance_sheet"))?
            .balance_sheet(instrument, quarterly)
            .await
    }
}

#[async_trait]
impl CashflowProvider for CachingConnector {
    async fn cashflow(
        &self,
        instrument: &Instrument,
        quarterly: bool,
    ) -> Result<Vec<CashflowRow>, BorsaError> {
        if let Some(store) = &self.stores.cashflow {
            let key = BoolByInstrumentKey {
                inst: InstrumentKey::from(instrument),
                flag: quarterly,
            };
            if let Some(v) = store.get(&key).await {
                return Ok((*v).clone());
            }
            let inner = self
                .inner
                .as_cashflow_provider()
                .ok_or_else(|| BorsaError::unsupported("cashflow"))?;
            let value = inner.cashflow(instrument, quarterly).await?;
            store.put(key, Arc::new(value.clone())).await;
            return Ok(value);
        }
        self.inner
            .as_cashflow_provider()
            .ok_or_else(|| BorsaError::unsupported("cashflow"))?
            .cashflow(instrument, quarterly)
            .await
    }
}

#[async_trait]
impl CalendarProvider for CachingConnector {
    async fn calendar(&self, instrument: &Instrument) -> Result<Calendar, BorsaError> {
        if let Some(store) = &self.stores.calendar {
            let key = InstrumentKey::from(instrument);
            if let Some(v) = store.get(&key).await {
                return Ok((*v).clone());
            }
            let inner = self
                .inner
                .as_calendar_provider()
                .ok_or_else(|| BorsaError::unsupported("calendar"))?;
            let value = inner.calendar(instrument).await?;
            store.put(key, Arc::new(value.clone())).await;
            return Ok(value);
        }
        self.inner
            .as_calendar_provider()
            .ok_or_else(|| BorsaError::unsupported("calendar"))?
            .calendar(instrument)
            .await
    }
}

#[async_trait]
impl RecommendationsProvider for CachingConnector {
    async fn recommendations(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<RecommendationRow>, BorsaError> {
        if let Some(store) = &self.stores.recommendations {
            let key = InstrumentKey::from(instrument);
            if let Some(v) = store.get(&key).await {
                return Ok((*v).clone());
            }
            let inner = self
                .inner
                .as_recommendations_provider()
                .ok_or_else(|| BorsaError::unsupported("recommendations"))?;
            let value = inner.recommendations(instrument).await?;
            store.put(key, Arc::new(value.clone())).await;
            return Ok(value);
        }
        self.inner
            .as_recommendations_provider()
            .ok_or_else(|| BorsaError::unsupported("recommendations"))?
            .recommendations(instrument)
            .await
    }
}

#[async_trait]
impl RecommendationsSummaryProvider for CachingConnector {
    async fn recommendations_summary(
        &self,
        instrument: &Instrument,
    ) -> Result<RecommendationSummary, BorsaError> {
        if let Some(store) = &self.stores.recommendations_summary {
            let key = InstrumentKey::from(instrument);
            if let Some(v) = store.get(&key).await {
                return Ok((*v).clone());
            }
            let inner = self
                .inner
                .as_recommendations_summary_provider()
                .ok_or_else(|| BorsaError::unsupported("recommendations_summary"))?;
            let value = inner.recommendations_summary(instrument).await?;
            store.put(key, Arc::new(value.clone())).await;
            return Ok(value);
        }
        self.inner
            .as_recommendations_summary_provider()
            .ok_or_else(|| BorsaError::unsupported("recommendations_summary"))?
            .recommendations_summary(instrument)
            .await
    }
}

#[async_trait]
impl UpgradesDowngradesProvider for CachingConnector {
    async fn upgrades_downgrades(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<UpgradeDowngradeRow>, BorsaError> {
        if let Some(store) = &self.stores.upgrades_downgrades {
            let key = InstrumentKey::from(instrument);
            if let Some(v) = store.get(&key).await {
                return Ok((*v).clone());
            }
            let inner = self
                .inner
                .as_upgrades_downgrades_provider()
                .ok_or_else(|| BorsaError::unsupported("upgrades_downgrades"))?;
            let value = inner.upgrades_downgrades(instrument).await?;
            store.put(key, Arc::new(value.clone())).await;
            return Ok(value);
        }
        self.inner
            .as_upgrades_downgrades_provider()
            .ok_or_else(|| BorsaError::unsupported("upgrades_downgrades"))?
            .upgrades_downgrades(instrument)
            .await
    }
}

#[async_trait]
impl AnalystPriceTargetProvider for CachingConnector {
    async fn analyst_price_target(
        &self,
        instrument: &Instrument,
    ) -> Result<PriceTarget, BorsaError> {
        if let Some(store) = &self.stores.analyst_price_target {
            let key = InstrumentKey::from(instrument);
            if let Some(v) = store.get(&key).await {
                return Ok((*v).clone());
            }
            let inner = self
                .inner
                .as_analyst_price_target_provider()
                .ok_or_else(|| BorsaError::unsupported("analyst_price_target"))?;
            let value = inner.analyst_price_target(instrument).await?;
            store.put(key, Arc::new(value.clone())).await;
            return Ok(value);
        }
        self.inner
            .as_analyst_price_target_provider()
            .ok_or_else(|| BorsaError::unsupported("analyst_price_target"))?
            .analyst_price_target(instrument)
            .await
    }
}

#[async_trait]
impl MajorHoldersProvider for CachingConnector {
    async fn major_holders(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<borsa_core::MajorHolder>, BorsaError> {
        if let Some(store) = &self.stores.major_holders {
            let key = InstrumentKey::from(instrument);
            if let Some(v) = store.get(&key).await {
                return Ok((*v).clone());
            }
            let inner = self
                .inner
                .as_major_holders_provider()
                .ok_or_else(|| BorsaError::unsupported("major_holders"))?;
            let value = inner.major_holders(instrument).await?;
            store.put(key, Arc::new(value.clone())).await;
            return Ok(value);
        }
        self.inner
            .as_major_holders_provider()
            .ok_or_else(|| BorsaError::unsupported("major_holders"))?
            .major_holders(instrument)
            .await
    }
}

#[async_trait]
impl InstitutionalHoldersProvider for CachingConnector {
    async fn institutional_holders(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<borsa_core::InstitutionalHolder>, BorsaError> {
        if let Some(store) = &self.stores.institutional_holders {
            let key = InstrumentKey::from(instrument);
            if let Some(v) = store.get(&key).await {
                return Ok((*v).clone());
            }
            let inner = self
                .inner
                .as_institutional_holders_provider()
                .ok_or_else(|| BorsaError::unsupported("institutional_holders"))?;
            let value = inner.institutional_holders(instrument).await?;
            store.put(key, Arc::new(value.clone())).await;
            return Ok(value);
        }
        self.inner
            .as_institutional_holders_provider()
            .ok_or_else(|| BorsaError::unsupported("institutional_holders"))?
            .institutional_holders(instrument)
            .await
    }
}

#[async_trait]
impl MutualFundHoldersProvider for CachingConnector {
    async fn mutual_fund_holders(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<borsa_core::InstitutionalHolder>, BorsaError> {
        if let Some(store) = &self.stores.mutual_fund_holders {
            let key = InstrumentKey::from(instrument);
            if let Some(v) = store.get(&key).await {
                return Ok((*v).clone());
            }
            let inner = self
                .inner
                .as_mutual_fund_holders_provider()
                .ok_or_else(|| BorsaError::unsupported("mutual_fund_holders"))?;
            let value = inner.mutual_fund_holders(instrument).await?;
            store.put(key, Arc::new(value.clone())).await;
            return Ok(value);
        }
        self.inner
            .as_mutual_fund_holders_provider()
            .ok_or_else(|| BorsaError::unsupported("mutual_fund_holders"))?
            .mutual_fund_holders(instrument)
            .await
    }
}

#[async_trait]
impl InsiderTransactionsProvider for CachingConnector {
    async fn insider_transactions(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<borsa_core::InsiderTransaction>, BorsaError> {
        if let Some(store) = &self.stores.insider_transactions {
            let key = InstrumentKey::from(instrument);
            if let Some(v) = store.get(&key).await {
                return Ok((*v).clone());
            }
            let inner = self
                .inner
                .as_insider_transactions_provider()
                .ok_or_else(|| BorsaError::unsupported("insider_transactions"))?;
            let value = inner.insider_transactions(instrument).await?;
            store.put(key, Arc::new(value.clone())).await;
            return Ok(value);
        }
        self.inner
            .as_insider_transactions_provider()
            .ok_or_else(|| BorsaError::unsupported("insider_transactions"))?
            .insider_transactions(instrument)
            .await
    }
}

#[async_trait]
impl InsiderRosterHoldersProvider for CachingConnector {
    async fn insider_roster_holders(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<borsa_core::InsiderRosterHolder>, BorsaError> {
        if let Some(store) = &self.stores.insider_roster {
            let key = InstrumentKey::from(instrument);
            if let Some(v) = store.get(&key).await {
                return Ok((*v).clone());
            }
            let inner = self
                .inner
                .as_insider_roster_holders_provider()
                .ok_or_else(|| BorsaError::unsupported("insider_roster_holders"))?;
            let value = inner.insider_roster_holders(instrument).await?;
            store.put(key, Arc::new(value.clone())).await;
            return Ok(value);
        }
        self.inner
            .as_insider_roster_holders_provider()
            .ok_or_else(|| BorsaError::unsupported("insider_roster_holders"))?
            .insider_roster_holders(instrument)
            .await
    }
}

#[async_trait]
impl NetSharePurchaseActivityProvider for CachingConnector {
    async fn net_share_purchase_activity(
        &self,
        instrument: &Instrument,
    ) -> Result<Option<borsa_core::NetSharePurchaseActivity>, BorsaError> {
        if let Some(store) = &self.stores.net_share_purchase_activity {
            let key = InstrumentKey::from(instrument);
            if let Some(v) = store.get(&key).await {
                return Ok((*v).clone());
            }
            let inner = self
                .inner
                .as_net_share_purchase_activity_provider()
                .ok_or_else(|| BorsaError::unsupported("net_share_purchase_activity"))?;
            let value = inner.net_share_purchase_activity(instrument).await?;
            store.put(key, Arc::new(value.clone())).await;
            return Ok(value);
        }
        self.inner
            .as_net_share_purchase_activity_provider()
            .ok_or_else(|| BorsaError::unsupported("net_share_purchase_activity"))?
            .net_share_purchase_activity(instrument)
            .await
    }
}

#[async_trait]
impl EsgProvider for CachingConnector {
    async fn sustainability(&self, instrument: &Instrument) -> Result<EsgScores, BorsaError> {
        if let Some(store) = &self.stores.esg {
            let key = InstrumentKey::from(instrument);
            if let Some(v) = store.get(&key).await {
                return Ok((*v).clone());
            }
            let inner = self
                .inner
                .as_esg_provider()
                .ok_or_else(|| BorsaError::unsupported("sustainability"))?;
            let value = inner.sustainability(instrument).await?;
            store.put(key, Arc::new(value.clone())).await;
            return Ok(value);
        }
        self.inner
            .as_esg_provider()
            .ok_or_else(|| BorsaError::unsupported("sustainability"))?
            .sustainability(instrument)
            .await
    }
}

#[async_trait]
impl NewsProvider for CachingConnector {
    async fn news(
        &self,
        instrument: &Instrument,
        req: NewsRequest,
    ) -> Result<Vec<NewsArticle>, BorsaError> {
        if let Some(store) = &self.stores.news {
            let key = NewsKey {
                inst: InstrumentKey::from(instrument),
                count: req.count,
                tab: NewsTabKey(req.tab),
            };
            if let Some(v) = store.get(&key).await {
                return Ok((*v).clone());
            }
            let inner = self
                .inner
                .as_news_provider()
                .ok_or_else(|| BorsaError::unsupported("news"))?;
            let value = inner.news(instrument, req).await?;
            store.put(key, Arc::new(value.clone())).await;
            return Ok(value);
        }
        self.inner
            .as_news_provider()
            .ok_or_else(|| BorsaError::unsupported("news"))?
            .news(instrument, req)
            .await
    }
}

#[async_trait]
impl StreamProvider for CachingConnector {
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
        let inner = self
            .inner
            .as_stream_provider()
            .ok_or_else(|| BorsaError::unsupported("stream_quotes"))?;
        inner.stream_quotes(instruments).await
    }
}

#[async_trait]
impl OptionsExpirationsProvider for CachingConnector {
    async fn options_expirations(&self, instrument: &Instrument) -> Result<Vec<i64>, BorsaError> {
        if let Some(store) = &self.stores.options_expirations {
            let key = InstrumentKey::from(instrument);
            if let Some(v) = store.get(&key).await {
                return Ok((*v).clone());
            }
            let inner = self
                .inner
                .as_options_expirations_provider()
                .ok_or_else(|| BorsaError::unsupported("options_expirations"))?;
            let value = inner.options_expirations(instrument).await?;
            store.put(key, Arc::new(value.clone())).await;
            return Ok(value);
        }
        self.inner
            .as_options_expirations_provider()
            .ok_or_else(|| BorsaError::unsupported("options_expirations"))?
            .options_expirations(instrument)
            .await
    }
}

#[async_trait]
impl OptionChainProvider for CachingConnector {
    async fn option_chain(
        &self,
        instrument: &Instrument,
        date: Option<i64>,
    ) -> Result<OptionChain, BorsaError> {
        if let Some(store) = &self.stores.option_chain {
            let key = OptionChainKey {
                inst: InstrumentKey::from(instrument),
                date,
            };
            if let Some(v) = store.get(&key).await {
                return Ok((*v).clone());
            }
            let inner = self
                .inner
                .as_option_chain_provider()
                .ok_or_else(|| BorsaError::unsupported("option_chain"))?;
            let value = inner.option_chain(instrument, date).await?;
            store.put(key, Arc::new(value.clone())).await;
            return Ok(value);
        }
        self.inner
            .as_option_chain_provider()
            .ok_or_else(|| BorsaError::unsupported("option_chain"))?
            .option_chain(instrument, date)
            .await
    }
}

#[async_trait]
impl SearchProvider for CachingConnector {
    async fn search(&self, req: SearchRequest) -> Result<SearchResponse, BorsaError> {
        if let Some(store) = &self.stores.search {
            let key = SearchKey {
                query: req.query().to_string(),
                kind: req.kind(),
                limit: req.limit(),
            };
            if let Some(v) = store.get(&key).await {
                return Ok((*v).clone());
            }
            let inner = self
                .inner
                .as_search_provider()
                .ok_or_else(|| BorsaError::unsupported("search"))?;
            let value = inner.search(req).await?;
            store.put(key, Arc::new(value.clone())).await;
            return Ok(value);
        }
        self.inner
            .as_search_provider()
            .ok_or_else(|| BorsaError::unsupported("search"))?
            .search(req)
            .await
    }
}
