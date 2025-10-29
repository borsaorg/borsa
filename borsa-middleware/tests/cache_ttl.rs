use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use borsa_core::{AssetKind, BorsaConnector, Instrument, connector::QuoteProvider};
use borsa_middleware::ConnectorBuilder;
use borsa_mock::MockConnector;
use borsa_types::CacheConfig;

struct CountingQuoteConnector {
    inner: Arc<dyn BorsaConnector>,
    count: Arc<AtomicUsize>,
}

impl CountingQuoteConnector {
    fn new(inner: Arc<dyn BorsaConnector>, count: Arc<AtomicUsize>) -> Self {
        Self { inner, count }
    }
}

#[async_trait::async_trait]
impl BorsaConnector for CountingQuoteConnector {
    fn name(&self) -> &'static str {
        "counting"
    }
    fn vendor(&self) -> &'static str {
        "test"
    }
    fn supports_kind(&self, _k: AssetKind) -> bool {
        true
    }
    fn as_quote_provider(&self) -> Option<&dyn QuoteProvider> {
        Some(self as &dyn QuoteProvider)
    }
}

#[async_trait::async_trait]
impl QuoteProvider for CountingQuoteConnector {
    async fn quote(
        &self,
        instrument: &borsa_core::Instrument,
    ) -> Result<borsa_core::Quote, borsa_core::BorsaError> {
        self.count.fetch_add(1, Ordering::SeqCst);
        self.inner
            .as_quote_provider()
            .unwrap()
            .quote(instrument)
            .await
    }
}

fn cfg(ms: u64) -> CacheConfig {
    let mut cfg = CacheConfig::default();
    cfg.per_capability_ttl_ms.insert("quote".into(), ms);
    cfg
}

#[tokio::test]
async fn ttl_expiration_causes_refetch() {
    let inner: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());
    let count = Arc::new(AtomicUsize::new(0));
    let raw: Arc<dyn BorsaConnector> = Arc::new(CountingQuoteConnector::new(inner, count.clone()));

    let cfg = cfg(50);
    let wrapped = ConnectorBuilder::new(raw).with_cache(&cfg).build().unwrap();
    let q = wrapped.as_quote_provider().unwrap();
    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).unwrap();

    let _ = q.quote(&inst).await.unwrap(); // miss -> fetch
    assert_eq!(count.load(Ordering::SeqCst), 1);
    let _ = q.quote(&inst).await.unwrap(); // hit
    assert_eq!(count.load(Ordering::SeqCst), 1);
    tokio::time::sleep(std::time::Duration::from_millis(60)).await;
    let _ = q.quote(&inst).await.unwrap(); // expired -> refetch
    assert_eq!(count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn ttl_zero_disables_caching() {
    let inner: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());
    let count = Arc::new(AtomicUsize::new(0));
    let raw: Arc<dyn BorsaConnector> = Arc::new(CountingQuoteConnector::new(inner, count.clone()));

    let cfg = cfg(0);
    let wrapped = ConnectorBuilder::new(raw).with_cache(&cfg).build().unwrap();
    let q = wrapped.as_quote_provider().unwrap();
    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).unwrap();

    let _ = q.quote(&inst).await.unwrap();
    let _ = q.quote(&inst).await.unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 2, "no caching when ttl=0");
}
