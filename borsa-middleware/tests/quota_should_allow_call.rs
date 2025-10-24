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

fn make_wrapper_spread(limit: u64, window_ms: u64) -> Arc<QuotaAwareConnector> {
    let inner: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());
    let cfg = QuotaConfig {
        limit,
        window: Duration::from_millis(window_ms),
        strategy: QuotaConsumptionStrategy::EvenSpreadHourly,
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

#[test]
fn hourly_spread_temporarily_blocks() {
    // For deterministic tests, use a small window so slice duration is small
    // Window 2400ms -> 24 slices -> slice = 100ms; limit 24 -> 1 per slice
    let wrapper = make_wrapper_spread(24, 2400);

    // First call in slice allowed
    assert!(wrapper.should_allow_call().is_ok());
    // Second call within same slice should be blocked with temporary QuotaExceeded
    let err = wrapper
        .should_allow_call()
        .expect_err("should block per-slice");
    if let borsa_core::BorsaError::QuotaExceeded {
        remaining,
        reset_in_ms,
    } = err
    {
        assert!(remaining > 0, "daily should still have remaining units");
        assert!(reset_in_ms <= 100, "should reset within slice duration");
    } else {
        panic!("unexpected error type");
    }

    // Wait for slice to roll
    std::thread::sleep(Duration::from_millis(120));
    assert!(wrapper.should_allow_call().is_ok());
}
