use std::sync::Arc;
use std::time::Duration;

use borsa_core::{AssetKind, BorsaConnector, BorsaError, Instrument, Interval};
use borsa_middleware::QuotaAwareConnector;
use borsa_mock::MockConnector;
use borsa_types::{QuotaConfig, QuotaConsumptionStrategy};

fn make_wrapper(limit: u64, window_ms: u64) -> Arc<QuotaAwareConnector> {
    let inner: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());
    let cfg = QuotaConfig {
        limit,
        window: Duration::from_millis(window_ms),
        strategy: QuotaConsumptionStrategy::Unit,
    };
    Arc::new(QuotaAwareConnector::new(inner, cfg))
}

fn make_wrapper_spread(limit: u64, window_ms: u64) -> Arc<QuotaAwareConnector> {
    let inner: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());
    let cfg = QuotaConfig {
        limit,
        window: Duration::from_millis(window_ms),
        strategy: QuotaConsumptionStrategy::EvenSpreadHourly,
    };
    Arc::new(QuotaAwareConnector::new(inner, cfg))
}

#[tokio::test]
async fn history_enforces_quota_limit() {
    let wrapper = make_wrapper(2, 86_400_000); // daily window

    let h = wrapper
        .as_history_provider()
        .expect("history capability present");
    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid symbol");
    let req = borsa_core::HistoryRequest::try_from_range(borsa_core::Range::D1, Interval::D1)
        .expect("valid req");

    // Within limit
    assert!(h.history(&inst, req.clone()).await.is_ok());
    assert!(h.history(&inst, req.clone()).await.is_ok());

    // Exceeds limit
    let err = h.history(&inst, req).await.expect_err("should error");
    assert!(matches!(err, BorsaError::QuotaExceeded { .. }));
}

#[tokio::test]
async fn quote_enforces_quota_limit() {
    let wrapper = make_wrapper(2, 86_400_000);

    let q = wrapper
        .as_quote_provider()
        .expect("quote capability present");
    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid symbol");

    // Within limit
    assert!(q.quote(&inst).await.is_ok());
    assert!(q.quote(&inst).await.is_ok());

    // Exceeds limit
    let err = q.quote(&inst).await.expect_err("should error");
    assert!(matches!(err, BorsaError::QuotaExceeded { .. }));
}

#[tokio::test]
async fn quote_temporarily_blocks_per_hour_slice() {
    // 2400ms window => 24 slices => ~100ms per slice; 24 limit => 1 per slice
    let wrapper = make_wrapper_spread(24, 2400);
    let q = wrapper.as_quote_provider().expect("quote present");
    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid symbol");

    assert!(q.quote(&inst).await.is_ok());
    let err = q.quote(&inst).await.expect_err("should block per-slice");
    assert!(matches!(err, BorsaError::QuotaExceeded { .. }));

    tokio::time::sleep(Duration::from_millis(120)).await;
    assert!(q.quote(&inst).await.is_ok());
}

#[tokio::test]
async fn profile_enforces_quota_limit() {
    let wrapper = make_wrapper(1, 86_400_000);

    let p = wrapper
        .as_profile_provider()
        .expect("profile capability present");
    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid symbol");

    assert!(p.profile(&inst).await.is_ok());
    let err = p.profile(&inst).await.expect_err("should error");
    assert!(matches!(err, BorsaError::QuotaExceeded { .. }));
}

#[tokio::test]
async fn search_enforces_quota_limit() {
    let wrapper = make_wrapper(1, 86_400_000);
    let s = wrapper
        .as_search_provider()
        .expect("search capability present");
    let req = borsa_core::SearchRequest::builder("tesla").build().unwrap();

    assert!(s.search(req.clone()).await.is_ok());
    let err = s.search(req).await.expect_err("should error");
    assert!(matches!(err, BorsaError::QuotaExceeded { .. }));
}

#[tokio::test]
async fn earnings_enforces_quota_limit() {
    let wrapper = make_wrapper(1, 86_400_000);
    let e = wrapper
        .as_earnings_provider()
        .expect("earnings capability present");
    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid symbol");

    assert!(e.earnings(&inst).await.is_ok());
    let err = e.earnings(&inst).await.expect_err("should error");
    assert!(matches!(err, BorsaError::QuotaExceeded { .. }));
}
