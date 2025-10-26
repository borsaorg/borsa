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

#[tokio::test]
async fn quote_maps_provider_rate_limit_to_wrapper_rate_limit() {
    let wrapper = make_wrapper(10, 60_000);
    let q = wrapper
        .as_quote_provider()
        .expect("quote capability present");

    let inst = Instrument::from_symbol("RATELIMIT", AssetKind::Equity).expect("valid symbol");
    let err = q.quote(&inst).await.expect_err("should map to rate limit");
    assert!(matches!(err, BorsaError::RateLimitExceeded { .. }));
}

#[tokio::test]
async fn history_maps_provider_rate_limit_to_wrapper_rate_limit() {
    let wrapper = make_wrapper(10, 60_000);
    let h = wrapper
        .as_history_provider()
        .expect("history capability present");

    let inst = Instrument::from_symbol("RATELIMIT", AssetKind::Equity).expect("valid symbol");
    let req = borsa_core::HistoryRequest::try_from_range(borsa_core::Range::D1, Interval::D1)
        .expect("valid req");
    let err = h
        .history(&inst, req)
        .await
        .expect_err("should map to rate limit");
    assert!(matches!(err, BorsaError::RateLimitExceeded { .. }));
}
