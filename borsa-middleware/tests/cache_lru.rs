use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

use borsa_core::{AssetKind, BorsaConnector, Instrument, connector::QuoteProvider};
use borsa_middleware::ConnectorBuilder;
use borsa_types::CacheConfig;
use borsa_mock::MockConnector;

struct CountingQuoteConnector {
    inner: Arc<dyn BorsaConnector>,
    count: Arc<AtomicUsize>,
}

impl CountingQuoteConnector {
    fn new(inner: Arc<dyn BorsaConnector>, count: Arc<AtomicUsize>) -> Self { Self { inner, count } }
}

#[async_trait::async_trait]
impl BorsaConnector for CountingQuoteConnector {
    fn name(&self) -> &'static str { "counting" }
    fn vendor(&self) -> &'static str { "test" }
    fn supports_kind(&self, _k: AssetKind) -> bool { true }
    fn as_quote_provider(&self) -> Option<&dyn QuoteProvider> { Some(self as &dyn QuoteProvider) }
}

#[async_trait::async_trait]
impl QuoteProvider for CountingQuoteConnector {
    async fn quote(&self, instrument: &borsa_core::Instrument) -> Result<borsa_core::Quote, borsa_core::BorsaError> {
        self.count.fetch_add(1, Ordering::SeqCst);
        self.inner.as_quote_provider().unwrap().quote(instrument).await
    }
}

#[tokio::test]
async fn lru_eviction_with_capacity_one() {
    let inner: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());
    let count = Arc::new(AtomicUsize::new(0));
    let raw: Arc<dyn BorsaConnector> = Arc::new(CountingQuoteConnector::new(inner, count.clone()));

    let mut cfg = CacheConfig::default();
    cfg.per_capability_ttl_ms.insert("quote".into(), 60_000);
    cfg.per_capability_max_entries.insert("quote".into(), 1);

    let wrapped = ConnectorBuilder::new(raw).with_cache(&cfg).build().unwrap();
    let q = wrapped.as_quote_provider().unwrap();

    let aapl = Instrument::from_symbol("AAPL", AssetKind::Equity).unwrap();
    let msft = Instrument::from_symbol("MSFT", AssetKind::Equity).unwrap();

    let _ = q.quote(&aapl).await.unwrap(); // miss -> fetch (1)
    let _ = q.quote(&msft).await.unwrap(); // miss -> fetch (2), evict AAPL (async)
    tokio::time::sleep(std::time::Duration::from_millis(20)).await; // allow eviction to complete
    let _ = q.quote(&aapl).await.unwrap(); // miss again due to eviction -> fetch (3)
    assert_eq!(count.load(Ordering::SeqCst), 3);
}


