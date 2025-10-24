use std::sync::Arc;
use std::time::Duration;

use borsa_core::{AssetKind, BorsaConnector, BorsaError, Instrument, Interval};
use borsa_middleware::QuotaAwareConnector;
use borsa_mock::MockConnector;
use borsa_types::{QuotaConfig, QuotaConsumptionStrategy, QuotaState};

fn make_wrapper(limit: u64, window_ms: u64) -> Arc<QuotaAwareConnector> {
    let inner: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());
    let cfg = QuotaConfig {
        limit,
        window: Duration::from_millis(window_ms),
        strategy: QuotaConsumptionStrategy::Unit,
    };
    let st = QuotaState {
        limit: cfg.limit,
        remaining: cfg.limit,
        reset_in: cfg.window,
    };
    Arc::new(QuotaAwareConnector::new(inner, cfg, st))
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
