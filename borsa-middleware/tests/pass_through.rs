use std::sync::Arc;

use borsa_core::{AssetKind, BorsaConnector, Instrument, Interval};
use borsa_middleware::QuotaAwareConnector;
use borsa_mock::MockConnector;
use borsa_types::{QuotaConfig, QuotaConsumptionStrategy, QuotaState};

fn default_quota() -> (QuotaConfig, QuotaState) {
    let cfg = QuotaConfig {
        limit: 10,
        window: std::time::Duration::from_secs(60),
        strategy: QuotaConsumptionStrategy::Unit,
    };
    let st = QuotaState {
        limit: cfg.limit,
        remaining: cfg.limit,
        reset_in: cfg.window,
    };
    (cfg, st)
}

#[tokio::test]
async fn forwards_name_and_vendor() {
    let inner: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());
    let (cfg, st) = default_quota();
    let wrapper = QuotaAwareConnector::new(inner.clone(), cfg, st);

    assert_eq!(wrapper.name(), inner.name());
    assert_eq!(wrapper.vendor(), inner.vendor());
}

#[tokio::test]
async fn forwards_capability_accessors() {
    let inner: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());
    let (cfg, st) = default_quota();
    let wrapper = QuotaAwareConnector::new(inner.clone(), cfg, st);

    assert!(wrapper.supports_kind(AssetKind::Equity));
    assert!(wrapper.as_quote_provider().is_some());
    assert!(wrapper.as_history_provider().is_some());
    assert!(wrapper.as_search_provider().is_some());
}

#[tokio::test]
async fn forwards_methods_calls() {
    let inner: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());
    let (cfg, st) = default_quota();
    let wrapper = Arc::new(QuotaAwareConnector::new(inner.clone(), cfg, st));

    let q = wrapper.as_quote_provider().unwrap();
    let h = wrapper.as_history_provider().unwrap();
    let s = wrapper.as_search_provider().unwrap();

    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid symbol");
    let quote = q.quote(&inst).await.expect("quote ok");
    assert_eq!(quote.symbol.as_str(), "AAPL");

    let hist_req =
        borsa_core::HistoryRequest::try_from_range(borsa_core::Range::D1, Interval::D1).unwrap();
    let hist = h.history(&inst, hist_req).await.expect("history ok");
    assert!(!hist.candles.is_empty());
    let supported = h.supported_history_intervals(AssetKind::Equity);
    assert!(supported.contains(&Interval::D1));

    let search_req = borsa_core::SearchRequest::builder("tesla").build().unwrap();
    let search = s.search(search_req).await.expect("search ok");
    assert!(!search.results.is_empty());
}
