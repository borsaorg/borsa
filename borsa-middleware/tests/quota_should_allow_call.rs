use std::sync::Arc;
use std::time::Duration;

use borsa_core::BorsaConnector;
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

#[test]
fn greedy_allows_until_limit_then_blocks() {
    let wrapper = make_wrapper(3, 10_000);

    assert!(wrapper.should_allow_call().is_ok());
    assert!(wrapper.should_allow_call().is_ok());
    assert!(wrapper.should_allow_call().is_ok());
    assert!(wrapper.should_allow_call().is_err());
}

#[test]
fn window_reset_allows_after_duration() {
    let wrapper = make_wrapper(2, 50);

    assert!(wrapper.should_allow_call().is_ok());
    assert!(wrapper.should_allow_call().is_ok());
    assert!(wrapper.should_allow_call().is_err());

    std::thread::sleep(Duration::from_millis(60));

    assert!(wrapper.should_allow_call().is_ok());
    assert!(wrapper.should_allow_call().is_ok());
    assert!(wrapper.should_allow_call().is_err());
}
