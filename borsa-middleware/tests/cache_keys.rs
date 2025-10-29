use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

use borsa_core::{AssetKind, BorsaConnector, Exchange, Instrument, connector::QuoteProvider};
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
async fn quote_cache_key_includes_exchange_and_kind() {
    let inner: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());
    let count = Arc::new(AtomicUsize::new(0));
    let raw: Arc<dyn BorsaConnector> = Arc::new(CountingQuoteConnector::new(inner, count.clone()));

    let mut cfg = CacheConfig::default();
    cfg.per_capability_ttl_ms.insert("quote".into(), 60_000);
    let wrapped = ConnectorBuilder::new(raw).with_cache(&cfg).build().unwrap();
    let q = wrapped.as_quote_provider().unwrap();

    let aapl_nyse = Instrument::from_symbol_and_exchange("AAPL", Exchange::NYSE, AssetKind::Equity).unwrap();
    let aapl_nasdaq = Instrument::from_symbol_and_exchange("AAPL", Exchange::NASDAQ, AssetKind::Equity).unwrap();

    let _ = q.quote(&aapl_nyse).await.unwrap();
    let _ = q.quote(&aapl_nasdaq).await.unwrap(); // should NOT hit cache from NYSE

    assert_eq!(count.load(Ordering::SeqCst), 2);
}



