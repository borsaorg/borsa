use std::future::Future;
use std::pin::Pin;
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
use moka::future::Cache;
#[cfg(feature = "tracing")]
use tracing::{debug, info, warn};

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

type CacheStoreFuture<V> = Pin<Box<dyn Future<Output = Result<V, BorsaError>> + Send>>;
type CacheLoader<K, V> = Arc<dyn Fn(K) -> CacheStoreFuture<V> + Send + Sync>;

#[async_trait]
trait CacheStore<K, V>: Send + Sync {
    async fn get_or_try_put_with(&self, key: K, loader: CacheLoader<K, V>)
    -> Result<V, BorsaError>;

    /// Return a value if already present in the cache without invoking a loader.
    async fn get_if_present(&self, key: &K) -> Option<V>;

    /// Insert a value into the cache with the store's configured TTL.
    async fn insert(&self, key: K, value: V);
}

struct MokaStore<K, V> {
    cache: Cache<K, V>,
    #[cfg(feature = "tracing")]
    ttl: std::time::Duration,
}

impl<K, V> MokaStore<K, V>
where
    K: Clone + std::hash::Hash + Eq + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    fn new(capacity: usize, ttl: Duration) -> Self {
        let cap = capacity.max(1);
        let cap_u64 = u64::try_from(cap).unwrap_or(u64::MAX);
        let cache = Cache::builder()
            .max_capacity(cap_u64)
            .time_to_live(ttl)
            .build();
        #[cfg(feature = "tracing")]
        {
            let ttl_ms: u64 = ttl.as_millis().try_into().unwrap_or(u64::MAX);
            info!(
                target = "borsa::middleware::cache",
                event = "store_init",
                max_capacity = cap_u64,
                ttl_ms = ttl_ms,
                "initialized per-capability cache store"
            );
        }
        Self {
            cache,
            #[cfg(feature = "tracing")]
            ttl,
        }
    }
    async fn get_or_try_put_with(
        &self,
        key: K,
        loader: CacheLoader<K, V>,
    ) -> Result<V, BorsaError> {
        #[cfg(feature = "tracing")]
        let did_load = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        #[cfg(feature = "tracing")]
        let loaded_flag = did_load.clone();

        #[cfg(feature = "tracing")]
        let wrapped_loader: CacheLoader<K, V> = Arc::new(move |k: K| {
            loaded_flag.store(true, std::sync::atomic::Ordering::Relaxed);
            loader(k)
        });

        #[cfg(not(feature = "tracing"))]
        let wrapped_loader = loader;

        let future = wrapped_loader(key.clone());
        let res = self
            .cache
            .try_get_with(key.clone(), future)
            .await
            .map_err(|err| (*err).clone());

        #[cfg(feature = "tracing")]
        match &res {
            Ok(_) => {
                let is_miss_insert = did_load.load(std::sync::atomic::Ordering::Relaxed);
                let ttl_ms: u64 = self.ttl.as_millis().try_into().unwrap_or(u64::MAX);
                if is_miss_insert {
                    debug!(
                        target = "borsa::middleware::cache",
                        event = "insert",
                        ttl_ms = ttl_ms,
                        "cache miss -> loaded and inserted"
                    );
                } else {
                    debug!(
                        target = "borsa::middleware::cache",
                        event = "hit",
                        "cache hit"
                    );
                }
            }
            Err(err) => {
                warn!(
                    target = "borsa::middleware::cache",
                    event = "error",
                    %err,
                    "cache lookup failed"
                );
            }
        }

        res
    }
}

#[async_trait]
impl<K, V> CacheStore<K, V> for MokaStore<K, V>
where
    K: Clone + std::hash::Hash + Eq + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    async fn get_or_try_put_with(
        &self,
        key: K,
        loader: CacheLoader<K, V>,
    ) -> Result<V, BorsaError> {
        self.get_or_try_put_with(key, loader).await
    }

    async fn get_if_present(&self, key: &K) -> Option<V> {
        self.cache.get(key).await
    }

    async fn insert(&self, key: K, value: V) {
        self.cache.insert(key, value).await;
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
        #[cfg(feature = "tracing")]
        {
            let default_ttl_ms = cfg.default_ttl_ms;
            let default_max_entries = cfg.default_max_entries;
            info!(
                target = "borsa::middleware::cache",
                event = "apply",
                default_ttl_ms = default_ttl_ms,
                default_max_entries = default_max_entries,
                overrides_ttl = cfg.per_capability_ttl_ms.len(),
                overrides_capacity = cfg.per_capability_max_entries.len(),
                "applying cache middleware"
            );
        }
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
            "default_negative_ttl_ms": self.cfg.default_negative_ttl_ms,
            "per_capability_negative_ttl_ms": self.cfg.per_capability_negative_ttl_ms,
        })
    }
}

// Per-capability typed stores; `None` means disabled (e.g., TTL=0).
struct Stores {
    // positive caches
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

    // negative caches (permanent errors)
    quote_neg: Option<Arc<dyn CacheStore<InstrumentKey, Arc<BorsaError>>>>,
    profile_neg: Option<Arc<dyn CacheStore<InstrumentKey, Arc<BorsaError>>>>,
    isin_neg: Option<Arc<dyn CacheStore<InstrumentKey, Arc<BorsaError>>>>,
    history_neg: Option<Arc<dyn CacheStore<HistoryKey, Arc<BorsaError>>>>,
    earnings_neg: Option<Arc<dyn CacheStore<InstrumentKey, Arc<BorsaError>>>>,
    income_stmt_neg: Option<Arc<dyn CacheStore<BoolByInstrumentKey, Arc<BorsaError>>>>,
    balance_sheet_neg: Option<Arc<dyn CacheStore<BoolByInstrumentKey, Arc<BorsaError>>>>,
    cashflow_neg: Option<Arc<dyn CacheStore<BoolByInstrumentKey, Arc<BorsaError>>>>,
    calendar_neg: Option<Arc<dyn CacheStore<InstrumentKey, Arc<BorsaError>>>>,
    recommendations_neg: Option<Arc<dyn CacheStore<InstrumentKey, Arc<BorsaError>>>>,
    recommendations_summary_neg: Option<Arc<dyn CacheStore<InstrumentKey, Arc<BorsaError>>>>,
    upgrades_downgrades_neg: Option<Arc<dyn CacheStore<InstrumentKey, Arc<BorsaError>>>>,
    analyst_price_target_neg: Option<Arc<dyn CacheStore<InstrumentKey, Arc<BorsaError>>>>,
    major_holders_neg: Option<Arc<dyn CacheStore<InstrumentKey, Arc<BorsaError>>>>,
    institutional_holders_neg: Option<Arc<dyn CacheStore<InstrumentKey, Arc<BorsaError>>>>,
    mutual_fund_holders_neg: Option<Arc<dyn CacheStore<InstrumentKey, Arc<BorsaError>>>>,
    insider_transactions_neg: Option<Arc<dyn CacheStore<InstrumentKey, Arc<BorsaError>>>>,
    insider_roster_neg: Option<Arc<dyn CacheStore<InstrumentKey, Arc<BorsaError>>>>,
    net_share_purchase_activity_neg: Option<Arc<dyn CacheStore<InstrumentKey, Arc<BorsaError>>>>,
    esg_neg: Option<Arc<dyn CacheStore<InstrumentKey, Arc<BorsaError>>>>,
    news_neg: Option<Arc<dyn CacheStore<NewsKey, Arc<BorsaError>>>>,
    options_expirations_neg: Option<Arc<dyn CacheStore<InstrumentKey, Arc<BorsaError>>>>,
    option_chain_neg: Option<Arc<dyn CacheStore<OptionChainKey, Arc<BorsaError>>>>,
    search_neg: Option<Arc<dyn CacheStore<SearchKey, Arc<BorsaError>>>>,
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
        let store = MokaStore::<K, V>::new(capacity, ttl);
        #[cfg(feature = "tracing")]
        {
            let ttl_ms: u64 = ttl.as_millis().try_into().unwrap_or(u64::MAX);
            info!(
                target = "borsa::middleware::cache",
                event = "store_create",
                capability = %cap,
                capacity = capacity,
                ttl_ms = ttl_ms,
                "created per-capability store"
            );
        }
        Some(Arc::new(store))
    }

    fn maybe_negative_store<K>(
        cfg: &CacheConfig,
        cap: Capability,
    ) -> Option<Arc<dyn CacheStore<K, Arc<BorsaError>>>>
    where
        K: Clone + std::hash::Hash + Eq + Send + Sync + 'static,
    {
        let ttl = cfg.negative_ttl_for(cap)?;
        let capacity = cfg.capacity_for(cap);
        let store = MokaStore::<K, Arc<BorsaError>>::new(capacity, ttl);
        #[cfg(feature = "tracing")]
        {
            let ttl_ms: u64 = ttl.as_millis().try_into().unwrap_or(u64::MAX);
            info!(
                target = "borsa::middleware::cache",
                event = "store_create_neg",
                capability = %cap,
                capacity = capacity,
                ttl_ms = ttl_ms,
                "created per-capability negative store"
            );
        }
        Some(Arc::new(store))
    }

    async fn cached_or_load_neg<K, T>(
        pos: Option<&Arc<dyn CacheStore<K, Arc<T>>>>,
        neg: Option<&Arc<dyn CacheStore<K, Arc<BorsaError>>>>,
        key: K,
        loader: CacheLoader<K, Arc<T>>,
    ) -> Result<Arc<T>, BorsaError>
    where
        K: Clone + std::hash::Hash + Eq + Send + Sync + 'static,
        T: Clone + Send + Sync + 'static,
    {
        if let Some(neg_store) = neg
            && let Some(err) = neg_store.get_if_present(&key).await
        {
            return Err((*err).clone());
        }

        let res = if let Some(pos_store) = pos {
            pos_store
                .get_or_try_put_with(key.clone(), Arc::clone(&loader))
                .await
        } else {
            loader(key.clone()).await
        };

        match res {
            Ok(v) => Ok(v),
            Err(e) => {
                if e.is_permanent()
                    && let Some(neg_store) = neg
                {
                    neg_store.insert(key, Arc::new(e.clone())).await;
                }
                Err(e)
            }
        }
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

            quote_neg: Self::maybe_negative_store(cfg, Capability::Quote),
            profile_neg: Self::maybe_negative_store(cfg, Capability::Profile),
            isin_neg: Self::maybe_negative_store(cfg, Capability::Isin),
            history_neg: Self::maybe_negative_store(cfg, Capability::History),
            earnings_neg: Self::maybe_negative_store(cfg, Capability::Earnings),
            income_stmt_neg: Self::maybe_negative_store(cfg, Capability::IncomeStatement),
            balance_sheet_neg: Self::maybe_negative_store(cfg, Capability::BalanceSheet),
            cashflow_neg: Self::maybe_negative_store(cfg, Capability::Cashflow),
            calendar_neg: Self::maybe_negative_store(cfg, Capability::Calendar),
            recommendations_neg: Self::maybe_negative_store(cfg, Capability::Recommendations),
            recommendations_summary_neg: Self::maybe_negative_store(
                cfg,
                Capability::RecommendationsSummary,
            ),
            upgrades_downgrades_neg: Self::maybe_negative_store(
                cfg,
                Capability::UpgradesDowngrades,
            ),
            analyst_price_target_neg: Self::maybe_negative_store(
                cfg,
                Capability::AnalystPriceTarget,
            ),
            major_holders_neg: Self::maybe_negative_store(cfg, Capability::MajorHolders),
            institutional_holders_neg: Self::maybe_negative_store(
                cfg,
                Capability::InstitutionalHolders,
            ),
            mutual_fund_holders_neg: Self::maybe_negative_store(cfg, Capability::MutualFundHolders),
            insider_transactions_neg: Self::maybe_negative_store(
                cfg,
                Capability::InsiderTransactions,
            ),
            insider_roster_neg: Self::maybe_negative_store(cfg, Capability::InsiderRoster),
            net_share_purchase_activity_neg: Self::maybe_negative_store(
                cfg,
                Capability::NetSharePurchaseActivity,
            ),
            esg_neg: Self::maybe_negative_store(cfg, Capability::Esg),
            news_neg: Self::maybe_negative_store(cfg, Capability::News),
            options_expirations_neg: Self::maybe_negative_store(
                cfg,
                Capability::OptionsExpirations,
            ),
            option_chain_neg: Self::maybe_negative_store(cfg, Capability::OptionChain),
            search_neg: Self::maybe_negative_store(cfg, Capability::Search),
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
        let key = InstrumentKey::from(instrument);
        let inner = Arc::clone(&self.inner);
        let instrument_clone = instrument.clone();
        let loader: CacheLoader<InstrumentKey, Arc<Quote>> = Arc::new(move |_key| {
            let inner = Arc::clone(&inner);
            let instrument = instrument_clone.clone();
            Box::pin(async move {
                let provider = inner
                    .as_quote_provider()
                    .ok_or_else(|| BorsaError::unsupported("quote"))?;
                let value = provider.quote(&instrument).await?;
                Ok(Arc::new(value))
            })
        });

        let arc = Self::cached_or_load_neg(
            self.stores.quote.as_ref(),
            self.stores.quote_neg.as_ref(),
            key,
            loader,
        )
        .await?;
        Ok((*arc).clone())
    }
}

#[async_trait]
impl ProfileProvider for CachingConnector {
    async fn profile(&self, instrument: &Instrument) -> Result<Profile, BorsaError> {
        let key = InstrumentKey::from(instrument);
        let inner = Arc::clone(&self.inner);
        let instrument_clone = instrument.clone();
        let loader: CacheLoader<InstrumentKey, Arc<Profile>> = Arc::new(move |_key| {
            let inner = Arc::clone(&inner);
            let instrument = instrument_clone.clone();
            Box::pin(async move {
                let provider = inner
                    .as_profile_provider()
                    .ok_or_else(|| BorsaError::unsupported("profile"))?;
                let value = provider.profile(&instrument).await?;
                Ok(Arc::new(value))
            })
        });

        let arc = Self::cached_or_load_neg(
            self.stores.profile.as_ref(),
            self.stores.profile_neg.as_ref(),
            key,
            loader,
        )
        .await?;
        Ok((*arc).clone())
    }
}

#[async_trait]
impl IsinProvider for CachingConnector {
    async fn isin(&self, instrument: &Instrument) -> Result<Option<Isin>, BorsaError> {
        let key = InstrumentKey::from(instrument);
        let inner = Arc::clone(&self.inner);
        let instrument_clone = instrument.clone();
        let loader: CacheLoader<InstrumentKey, Arc<Option<Isin>>> = Arc::new(move |_key| {
            let inner = Arc::clone(&inner);
            let instrument = instrument_clone.clone();
            Box::pin(async move {
                let provider = inner
                    .as_isin_provider()
                    .ok_or_else(|| BorsaError::unsupported("isin"))?;
                let value = provider.isin(&instrument).await?;
                Ok(Arc::new(value))
            })
        });

        let arc = Self::cached_or_load_neg(
            self.stores.isin.as_ref(),
            self.stores.isin_neg.as_ref(),
            key,
            loader,
        )
        .await?;
        Ok((*arc).clone())
    }
}

#[async_trait]
impl HistoryProvider for CachingConnector {
    async fn history(
        &self,
        instrument: &Instrument,
        req: HistoryRequest,
    ) -> Result<HistoryResponse, BorsaError> {
        let key = HistoryKey::from_request(instrument, &req);
        let inner = Arc::clone(&self.inner);
        let instrument_clone = instrument.clone();
        let req_clone = req.clone();
        let loader: CacheLoader<HistoryKey, Arc<HistoryResponse>> = Arc::new(move |_key| {
            let inner = Arc::clone(&inner);
            let instrument = instrument_clone.clone();
            let request = req_clone.clone();
            Box::pin(async move {
                let provider = inner
                    .as_history_provider()
                    .ok_or_else(|| BorsaError::unsupported("history"))?;
                let value = provider.history(&instrument, request).await?;
                Ok(Arc::new(value))
            })
        });

        let arc = Self::cached_or_load_neg(
            self.stores.history.as_ref(),
            self.stores.history_neg.as_ref(),
            key,
            loader,
        )
        .await?;
        Ok((*arc).clone())
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
        let key = InstrumentKey::from(instrument);
        let inner = Arc::clone(&self.inner);
        let instrument_clone = instrument.clone();
        let loader: CacheLoader<InstrumentKey, Arc<Earnings>> = Arc::new(move |_key| {
            let inner = Arc::clone(&inner);
            let instrument = instrument_clone.clone();
            Box::pin(async move {
                let provider = inner
                    .as_earnings_provider()
                    .ok_or_else(|| BorsaError::unsupported("earnings"))?;
                let value = provider.earnings(&instrument).await?;
                Ok(Arc::new(value))
            })
        });

        let arc = Self::cached_or_load_neg(
            self.stores.earnings.as_ref(),
            self.stores.earnings_neg.as_ref(),
            key,
            loader,
        )
        .await?;
        Ok((*arc).clone())
    }
}

#[async_trait]
impl IncomeStatementProvider for CachingConnector {
    async fn income_statement(
        &self,
        instrument: &Instrument,
        quarterly: bool,
    ) -> Result<Vec<IncomeStatementRow>, BorsaError> {
        let key = BoolByInstrumentKey {
            inst: InstrumentKey::from(instrument),
            flag: quarterly,
        };
        let inner = Arc::clone(&self.inner);
        let instrument_clone = instrument.clone();
        let loader: CacheLoader<BoolByInstrumentKey, Arc<Vec<IncomeStatementRow>>> =
            Arc::new(move |_key| {
                let inner = Arc::clone(&inner);
                let instrument = instrument_clone.clone();
                Box::pin(async move {
                    let provider = inner
                        .as_income_statement_provider()
                        .ok_or_else(|| BorsaError::unsupported("income_statement"))?;
                    let value = provider.income_statement(&instrument, quarterly).await?;
                    Ok(Arc::new(value))
                })
            });

        let arc = Self::cached_or_load_neg(
            self.stores.income_stmt.as_ref(),
            self.stores.income_stmt_neg.as_ref(),
            key,
            loader,
        )
        .await?;
        Ok((*arc).clone())
    }
}

#[async_trait]
impl BalanceSheetProvider for CachingConnector {
    async fn balance_sheet(
        &self,
        instrument: &Instrument,
        quarterly: bool,
    ) -> Result<Vec<BalanceSheetRow>, BorsaError> {
        let key = BoolByInstrumentKey {
            inst: InstrumentKey::from(instrument),
            flag: quarterly,
        };
        let inner = Arc::clone(&self.inner);
        let instrument_clone = instrument.clone();
        let loader: CacheLoader<BoolByInstrumentKey, Arc<Vec<BalanceSheetRow>>> =
            Arc::new(move |_key| {
                let inner = Arc::clone(&inner);
                let instrument = instrument_clone.clone();
                Box::pin(async move {
                    let provider = inner
                        .as_balance_sheet_provider()
                        .ok_or_else(|| BorsaError::unsupported("balance_sheet"))?;
                    let value = provider.balance_sheet(&instrument, quarterly).await?;
                    Ok(Arc::new(value))
                })
            });

        let arc = Self::cached_or_load_neg(
            self.stores.balance_sheet.as_ref(),
            self.stores.balance_sheet_neg.as_ref(),
            key,
            loader,
        )
        .await?;
        Ok((*arc).clone())
    }
}

#[async_trait]
impl CashflowProvider for CachingConnector {
    async fn cashflow(
        &self,
        instrument: &Instrument,
        quarterly: bool,
    ) -> Result<Vec<CashflowRow>, BorsaError> {
        let key = BoolByInstrumentKey {
            inst: InstrumentKey::from(instrument),
            flag: quarterly,
        };
        let inner = Arc::clone(&self.inner);
        let instrument_clone = instrument.clone();
        let loader: CacheLoader<BoolByInstrumentKey, Arc<Vec<CashflowRow>>> =
            Arc::new(move |_key| {
                let inner = Arc::clone(&inner);
                let instrument = instrument_clone.clone();
                Box::pin(async move {
                    let provider = inner
                        .as_cashflow_provider()
                        .ok_or_else(|| BorsaError::unsupported("cashflow"))?;
                    let value = provider.cashflow(&instrument, quarterly).await?;
                    Ok(Arc::new(value))
                })
            });

        let arc = Self::cached_or_load_neg(
            self.stores.cashflow.as_ref(),
            self.stores.cashflow_neg.as_ref(),
            key,
            loader,
        )
        .await?;
        Ok((*arc).clone())
    }
}

#[async_trait]
impl CalendarProvider for CachingConnector {
    async fn calendar(&self, instrument: &Instrument) -> Result<Calendar, BorsaError> {
        let key = InstrumentKey::from(instrument);
        let inner = Arc::clone(&self.inner);
        let instrument_clone = instrument.clone();
        let loader: CacheLoader<InstrumentKey, Arc<Calendar>> = Arc::new(move |_key| {
            let inner = Arc::clone(&inner);
            let instrument = instrument_clone.clone();
            Box::pin(async move {
                let provider = inner
                    .as_calendar_provider()
                    .ok_or_else(|| BorsaError::unsupported("calendar"))?;
                let value = provider.calendar(&instrument).await?;
                Ok(Arc::new(value))
            })
        });

        let arc = Self::cached_or_load_neg(
            self.stores.calendar.as_ref(),
            self.stores.calendar_neg.as_ref(),
            key,
            loader,
        )
        .await?;
        Ok((*arc).clone())
    }
}

#[async_trait]
impl RecommendationsProvider for CachingConnector {
    async fn recommendations(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<RecommendationRow>, BorsaError> {
        let key = InstrumentKey::from(instrument);
        let inner = Arc::clone(&self.inner);
        let instrument_clone = instrument.clone();
        let loader: CacheLoader<InstrumentKey, Arc<Vec<RecommendationRow>>> =
            Arc::new(move |_key| {
                let inner = Arc::clone(&inner);
                let instrument = instrument_clone.clone();
                Box::pin(async move {
                    let provider = inner
                        .as_recommendations_provider()
                        .ok_or_else(|| BorsaError::unsupported("recommendations"))?;
                    let value = provider.recommendations(&instrument).await?;
                    Ok(Arc::new(value))
                })
            });

        let arc = Self::cached_or_load_neg(
            self.stores.recommendations.as_ref(),
            self.stores.recommendations_neg.as_ref(),
            key,
            loader,
        )
        .await?;
        Ok((*arc).clone())
    }
}

#[async_trait]
impl RecommendationsSummaryProvider for CachingConnector {
    async fn recommendations_summary(
        &self,
        instrument: &Instrument,
    ) -> Result<RecommendationSummary, BorsaError> {
        let key = InstrumentKey::from(instrument);
        let inner = Arc::clone(&self.inner);
        let instrument_clone = instrument.clone();
        let loader: CacheLoader<InstrumentKey, Arc<RecommendationSummary>> =
            Arc::new(move |_key| {
                let inner = Arc::clone(&inner);
                let instrument = instrument_clone.clone();
                Box::pin(async move {
                    let provider = inner
                        .as_recommendations_summary_provider()
                        .ok_or_else(|| BorsaError::unsupported("recommendations_summary"))?;
                    let value = provider.recommendations_summary(&instrument).await?;
                    Ok(Arc::new(value))
                })
            });

        let arc = Self::cached_or_load_neg(
            self.stores.recommendations_summary.as_ref(),
            self.stores.recommendations_summary_neg.as_ref(),
            key,
            loader,
        )
        .await?;
        Ok((*arc).clone())
    }
}

#[async_trait]
impl UpgradesDowngradesProvider for CachingConnector {
    async fn upgrades_downgrades(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<UpgradeDowngradeRow>, BorsaError> {
        let key = InstrumentKey::from(instrument);
        let inner = Arc::clone(&self.inner);
        let instrument_clone = instrument.clone();
        let loader: CacheLoader<InstrumentKey, Arc<Vec<UpgradeDowngradeRow>>> =
            Arc::new(move |_key| {
                let inner = Arc::clone(&inner);
                let instrument = instrument_clone.clone();
                Box::pin(async move {
                    let provider = inner
                        .as_upgrades_downgrades_provider()
                        .ok_or_else(|| BorsaError::unsupported("upgrades_downgrades"))?;
                    let value = provider.upgrades_downgrades(&instrument).await?;
                    Ok(Arc::new(value))
                })
            });

        let arc = Self::cached_or_load_neg(
            self.stores.upgrades_downgrades.as_ref(),
            self.stores.upgrades_downgrades_neg.as_ref(),
            key,
            loader,
        )
        .await?;
        Ok((*arc).clone())
    }
}

#[async_trait]
impl AnalystPriceTargetProvider for CachingConnector {
    async fn analyst_price_target(
        &self,
        instrument: &Instrument,
    ) -> Result<PriceTarget, BorsaError> {
        let key = InstrumentKey::from(instrument);
        let inner = Arc::clone(&self.inner);
        let instrument_clone = instrument.clone();
        let loader: CacheLoader<InstrumentKey, Arc<PriceTarget>> = Arc::new(move |_key| {
            let inner = Arc::clone(&inner);
            let instrument = instrument_clone.clone();
            Box::pin(async move {
                let provider = inner
                    .as_analyst_price_target_provider()
                    .ok_or_else(|| BorsaError::unsupported("analyst_price_target"))?;
                let value = provider.analyst_price_target(&instrument).await?;
                Ok(Arc::new(value))
            })
        });

        let arc = Self::cached_or_load_neg(
            self.stores.analyst_price_target.as_ref(),
            self.stores.analyst_price_target_neg.as_ref(),
            key,
            loader,
        )
        .await?;
        Ok((*arc).clone())
    }
}

#[async_trait]
impl MajorHoldersProvider for CachingConnector {
    async fn major_holders(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<borsa_core::MajorHolder>, BorsaError> {
        let key = InstrumentKey::from(instrument);
        let inner = Arc::clone(&self.inner);
        let instrument_clone = instrument.clone();
        let loader: CacheLoader<InstrumentKey, Arc<Vec<borsa_core::MajorHolder>>> =
            Arc::new(move |_key| {
                let inner = Arc::clone(&inner);
                let instrument = instrument_clone.clone();
                Box::pin(async move {
                    let provider = inner
                        .as_major_holders_provider()
                        .ok_or_else(|| BorsaError::unsupported("major_holders"))?;
                    let value = provider.major_holders(&instrument).await?;
                    Ok(Arc::new(value))
                })
            });

        let arc = Self::cached_or_load_neg(
            self.stores.major_holders.as_ref(),
            self.stores.major_holders_neg.as_ref(),
            key,
            loader,
        )
        .await?;
        Ok((*arc).clone())
    }
}

#[async_trait]
impl InstitutionalHoldersProvider for CachingConnector {
    async fn institutional_holders(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<borsa_core::InstitutionalHolder>, BorsaError> {
        let key = InstrumentKey::from(instrument);
        let inner = Arc::clone(&self.inner);
        let instrument_clone = instrument.clone();
        let loader: CacheLoader<InstrumentKey, Arc<Vec<borsa_core::InstitutionalHolder>>> =
            Arc::new(move |_key| {
                let inner = Arc::clone(&inner);
                let instrument = instrument_clone.clone();
                Box::pin(async move {
                    let provider = inner
                        .as_institutional_holders_provider()
                        .ok_or_else(|| BorsaError::unsupported("institutional_holders"))?;
                    let value = provider.institutional_holders(&instrument).await?;
                    Ok(Arc::new(value))
                })
            });

        let arc = Self::cached_or_load_neg(
            self.stores.institutional_holders.as_ref(),
            self.stores.institutional_holders_neg.as_ref(),
            key,
            loader,
        )
        .await?;
        Ok((*arc).clone())
    }
}

#[async_trait]
impl MutualFundHoldersProvider for CachingConnector {
    async fn mutual_fund_holders(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<borsa_core::InstitutionalHolder>, BorsaError> {
        let key = InstrumentKey::from(instrument);
        let inner = Arc::clone(&self.inner);
        let instrument_clone = instrument.clone();
        let loader: CacheLoader<InstrumentKey, Arc<Vec<borsa_core::InstitutionalHolder>>> =
            Arc::new(move |_key| {
                let inner = Arc::clone(&inner);
                let instrument = instrument_clone.clone();
                Box::pin(async move {
                    let provider = inner
                        .as_mutual_fund_holders_provider()
                        .ok_or_else(|| BorsaError::unsupported("mutual_fund_holders"))?;
                    let value = provider.mutual_fund_holders(&instrument).await?;
                    Ok(Arc::new(value))
                })
            });

        let arc = Self::cached_or_load_neg(
            self.stores.mutual_fund_holders.as_ref(),
            self.stores.mutual_fund_holders_neg.as_ref(),
            key,
            loader,
        )
        .await?;
        Ok((*arc).clone())
    }
}

#[async_trait]
impl InsiderTransactionsProvider for CachingConnector {
    async fn insider_transactions(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<borsa_core::InsiderTransaction>, BorsaError> {
        let key = InstrumentKey::from(instrument);
        let inner = Arc::clone(&self.inner);
        let instrument_clone = instrument.clone();
        let loader: CacheLoader<InstrumentKey, Arc<Vec<borsa_core::InsiderTransaction>>> =
            Arc::new(move |_key| {
                let inner = Arc::clone(&inner);
                let instrument = instrument_clone.clone();
                Box::pin(async move {
                    let provider = inner
                        .as_insider_transactions_provider()
                        .ok_or_else(|| BorsaError::unsupported("insider_transactions"))?;
                    let value = provider.insider_transactions(&instrument).await?;
                    Ok(Arc::new(value))
                })
            });

        let arc = Self::cached_or_load_neg(
            self.stores.insider_transactions.as_ref(),
            self.stores.insider_transactions_neg.as_ref(),
            key,
            loader,
        )
        .await?;
        Ok((*arc).clone())
    }
}

#[async_trait]
impl InsiderRosterHoldersProvider for CachingConnector {
    async fn insider_roster_holders(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<borsa_core::InsiderRosterHolder>, BorsaError> {
        let key = InstrumentKey::from(instrument);
        let inner = Arc::clone(&self.inner);
        let instrument_clone = instrument.clone();
        let loader: CacheLoader<InstrumentKey, Arc<Vec<borsa_core::InsiderRosterHolder>>> =
            Arc::new(move |_key| {
                let inner = Arc::clone(&inner);
                let instrument = instrument_clone.clone();
                Box::pin(async move {
                    let provider = inner
                        .as_insider_roster_holders_provider()
                        .ok_or_else(|| BorsaError::unsupported("insider_roster_holders"))?;
                    let value = provider.insider_roster_holders(&instrument).await?;
                    Ok(Arc::new(value))
                })
            });

        let arc = Self::cached_or_load_neg(
            self.stores.insider_roster.as_ref(),
            self.stores.insider_roster_neg.as_ref(),
            key,
            loader,
        )
        .await?;
        Ok((*arc).clone())
    }
}

#[async_trait]
impl NetSharePurchaseActivityProvider for CachingConnector {
    async fn net_share_purchase_activity(
        &self,
        instrument: &Instrument,
    ) -> Result<Option<borsa_core::NetSharePurchaseActivity>, BorsaError> {
        let key = InstrumentKey::from(instrument);
        let inner = Arc::clone(&self.inner);
        let instrument_clone = instrument.clone();
        let loader: CacheLoader<InstrumentKey, Arc<Option<borsa_core::NetSharePurchaseActivity>>> =
            Arc::new(move |_key| {
                let inner = Arc::clone(&inner);
                let instrument = instrument_clone.clone();
                Box::pin(async move {
                    let provider = inner
                        .as_net_share_purchase_activity_provider()
                        .ok_or_else(|| BorsaError::unsupported("net_share_purchase_activity"))?;
                    let value = provider.net_share_purchase_activity(&instrument).await?;
                    Ok(Arc::new(value))
                })
            });

        let arc = Self::cached_or_load_neg(
            self.stores.net_share_purchase_activity.as_ref(),
            self.stores.net_share_purchase_activity_neg.as_ref(),
            key,
            loader,
        )
        .await?;
        Ok((*arc).clone())
    }
}

#[async_trait]
impl EsgProvider for CachingConnector {
    async fn sustainability(&self, instrument: &Instrument) -> Result<EsgScores, BorsaError> {
        let key = InstrumentKey::from(instrument);
        let inner = Arc::clone(&self.inner);
        let instrument_clone = instrument.clone();
        let loader: CacheLoader<InstrumentKey, Arc<EsgScores>> = Arc::new(move |_key| {
            let inner = Arc::clone(&inner);
            let instrument = instrument_clone.clone();
            Box::pin(async move {
                let provider = inner
                    .as_esg_provider()
                    .ok_or_else(|| BorsaError::unsupported("sustainability"))?;
                let value = provider.sustainability(&instrument).await?;
                Ok(Arc::new(value))
            })
        });

        let arc = Self::cached_or_load_neg(
            self.stores.esg.as_ref(),
            self.stores.esg_neg.as_ref(),
            key,
            loader,
        )
        .await?;
        Ok((*arc).clone())
    }
}

#[async_trait]
impl NewsProvider for CachingConnector {
    async fn news(
        &self,
        instrument: &Instrument,
        req: NewsRequest,
    ) -> Result<Vec<NewsArticle>, BorsaError> {
        let key = NewsKey {
            inst: InstrumentKey::from(instrument),
            count: req.count,
            tab: NewsTabKey(req.tab),
        };
        let inner = Arc::clone(&self.inner);
        let instrument_clone = instrument.clone();
        let req_clone = req;
        let loader: CacheLoader<NewsKey, Arc<Vec<NewsArticle>>> = Arc::new(move |_key| {
            let inner = Arc::clone(&inner);
            let instrument = instrument_clone.clone();
            let request = req_clone;
            Box::pin(async move {
                let provider = inner
                    .as_news_provider()
                    .ok_or_else(|| BorsaError::unsupported("news"))?;
                let value = provider.news(&instrument, request).await?;
                Ok(Arc::new(value))
            })
        });

        let arc = Self::cached_or_load_neg(
            self.stores.news.as_ref(),
            self.stores.news_neg.as_ref(),
            key,
            loader,
        )
        .await?;
        Ok((*arc).clone())
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
        let key = InstrumentKey::from(instrument);
        let inner = Arc::clone(&self.inner);
        let instrument_clone = instrument.clone();
        let loader: CacheLoader<InstrumentKey, Arc<Vec<i64>>> = Arc::new(move |_key| {
            let inner = Arc::clone(&inner);
            let instrument = instrument_clone.clone();
            Box::pin(async move {
                let provider = inner
                    .as_options_expirations_provider()
                    .ok_or_else(|| BorsaError::unsupported("options_expirations"))?;
                let value = provider.options_expirations(&instrument).await?;
                Ok(Arc::new(value))
            })
        });

        let arc = Self::cached_or_load_neg(
            self.stores.options_expirations.as_ref(),
            self.stores.options_expirations_neg.as_ref(),
            key,
            loader,
        )
        .await?;
        Ok((*arc).clone())
    }
}

#[async_trait]
impl OptionChainProvider for CachingConnector {
    async fn option_chain(
        &self,
        instrument: &Instrument,
        date: Option<i64>,
    ) -> Result<OptionChain, BorsaError> {
        let key = OptionChainKey {
            inst: InstrumentKey::from(instrument),
            date,
        };
        let inner = Arc::clone(&self.inner);
        let instrument_clone = instrument.clone();
        let loader: CacheLoader<OptionChainKey, Arc<OptionChain>> = Arc::new(move |_key| {
            let inner = Arc::clone(&inner);
            let instrument = instrument_clone.clone();
            Box::pin(async move {
                let provider = inner
                    .as_option_chain_provider()
                    .ok_or_else(|| BorsaError::unsupported("option_chain"))?;
                let value = provider.option_chain(&instrument, date).await?;
                Ok(Arc::new(value))
            })
        });

        let arc = Self::cached_or_load_neg(
            self.stores.option_chain.as_ref(),
            self.stores.option_chain_neg.as_ref(),
            key,
            loader,
        )
        .await?;
        Ok((*arc).clone())
    }
}

#[async_trait]
impl SearchProvider for CachingConnector {
    async fn search(&self, req: SearchRequest) -> Result<SearchResponse, BorsaError> {
        let key = SearchKey {
            query: req.query().to_string(),
            kind: req.kind(),
            limit: req.limit(),
        };
        let inner = Arc::clone(&self.inner);
        let req_clone = req.clone();
        let loader: CacheLoader<SearchKey, Arc<SearchResponse>> = Arc::new(move |_key| {
            let inner = Arc::clone(&inner);
            let request = req_clone.clone();
            Box::pin(async move {
                let provider = inner
                    .as_search_provider()
                    .ok_or_else(|| BorsaError::unsupported("search"))?;
                let value = provider.search(request).await?;
                Ok(Arc::new(value))
            })
        });

        let arc = Self::cached_or_load_neg(
            self.stores.search.as_ref(),
            self.stores.search_neg.as_ref(),
            key,
            loader,
        )
        .await?;
        Ok((*arc).clone())
    }
}
