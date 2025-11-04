use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use borsa_core::{AssetKind, BorsaConnector, BorsaError, Instrument, connector::ProfileProvider};
use borsa_middleware::ConnectorBuilder;
use borsa_types::CacheConfig;

struct NotFoundProfileConnector {
    count: Arc<AtomicUsize>,
}

impl NotFoundProfileConnector {
    const fn new(count: Arc<AtomicUsize>) -> Self {
        Self { count }
    }
}

#[async_trait::async_trait]
impl BorsaConnector for NotFoundProfileConnector {
    fn name(&self) -> &'static str {
        "nf"
    }
    fn vendor(&self) -> &'static str {
        "test"
    }
    fn supports_kind(&self, _k: AssetKind) -> bool {
        true
    }
    fn as_profile_provider(&self) -> Option<&dyn ProfileProvider> {
        Some(self as &dyn ProfileProvider)
    }
}

#[async_trait::async_trait]
impl ProfileProvider for NotFoundProfileConnector {
    async fn profile(&self, instrument: &Instrument) -> Result<borsa_core::Profile, BorsaError> {
        self.count.fetch_add(1, Ordering::SeqCst);
        Err(BorsaError::not_found(format!(
            "profile for {}",
            instrument.symbol()
        )))
    }
}

fn cfg_with_negative_ttl(ms: u64) -> CacheConfig {
    let mut cfg = CacheConfig::default();
    // enable positive cache for profile capability (not used here but mirrors real config)
    cfg.per_capability_ttl_ms.insert("profile".into(), 60_000);
    cfg.default_negative_ttl_ms = ms;
    cfg
}

#[tokio::test]
async fn negative_permanent_errors_are_cached() {
    let count = Arc::new(AtomicUsize::new(0));
    let raw: Arc<dyn BorsaConnector> = Arc::new(NotFoundProfileConnector::new(count.clone()));
    let cfg = cfg_with_negative_ttl(200);
    let wrapped = ConnectorBuilder::new(raw).with_cache(&cfg).build().unwrap();
    let p = wrapped.as_profile_provider().unwrap();
    let inst = Instrument::from_symbol("BTC", AssetKind::Crypto).unwrap();

    // First call hits provider and returns NotFound
    assert!(matches!(
        p.profile(&inst).await,
        Err(BorsaError::NotFound { .. })
    ));
    assert_eq!(count.load(Ordering::SeqCst), 1);

    // Second call within negative TTL should NOT hit provider again
    assert!(matches!(
        p.profile(&inst).await,
        Err(BorsaError::NotFound { .. })
    ));
    assert_eq!(count.load(Ordering::SeqCst), 1);

    // After TTL, it should hit again
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    assert!(matches!(
        p.profile(&inst).await,
        Err(BorsaError::NotFound { .. })
    ));
    assert_eq!(count.load(Ordering::SeqCst), 2);
}

struct RateLimitedProfileConnector {
    count: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl BorsaConnector for RateLimitedProfileConnector {
    fn name(&self) -> &'static str {
        "rl"
    }
    fn vendor(&self) -> &'static str {
        "test"
    }
    fn supports_kind(&self, _k: AssetKind) -> bool {
        true
    }
    fn as_profile_provider(&self) -> Option<&dyn ProfileProvider> {
        Some(self as &dyn ProfileProvider)
    }
}

#[async_trait::async_trait]
impl ProfileProvider for RateLimitedProfileConnector {
    async fn profile(&self, _instrument: &Instrument) -> Result<borsa_core::Profile, BorsaError> {
        self.count.fetch_add(1, Ordering::SeqCst);
        Err(BorsaError::RateLimitExceeded {
            limit: 10,
            window_ms: 1000,
        })
    }
}

#[tokio::test]
async fn transient_errors_are_not_negative_cached() {
    let count = Arc::new(AtomicUsize::new(0));
    let raw: Arc<dyn BorsaConnector> = Arc::new(RateLimitedProfileConnector {
        count: count.clone(),
    });
    let mut cfg = CacheConfig::default();
    cfg.per_capability_ttl_ms.insert("profile".into(), 60_000);
    cfg.default_negative_ttl_ms = 60_000; // enable negative caching in general
    let wrapped = ConnectorBuilder::new(raw).with_cache(&cfg).build().unwrap();
    let p = wrapped.as_profile_provider().unwrap();
    let inst = Instrument::from_symbol("BTC", AssetKind::Crypto).unwrap();

    assert!(matches!(
        p.profile(&inst).await,
        Err(BorsaError::RateLimitExceeded { .. })
    ));
    assert!(matches!(
        p.profile(&inst).await,
        Err(BorsaError::RateLimitExceeded { .. })
    ));
    assert_eq!(
        count.load(Ordering::SeqCst),
        2,
        "transient errors must not be cached"
    );
}
