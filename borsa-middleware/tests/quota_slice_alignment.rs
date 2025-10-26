use borsa_core::BorsaConnector;
use borsa_middleware::QuotaAwareConnector;
use borsa_mock::MockConnector;
use borsa_types::{QuotaConfig, QuotaConsumptionStrategy};
use std::sync::Arc;
use std::time::Duration;

/// Verify that `EvenSpreadHourly` quota slices remain aligned to fixed time boundaries
/// even when calls are made with gaps spanning multiple slice durations.
///
/// This test ensures that after skipping multiple slice periods, subsequent calls
/// correctly reset to the boundary of the current slice rather than drifting to
/// arbitrary points in time.
#[test]
fn quota_spread_maintains_slice_boundary_alignment() {
    // Configure quota: 24 slices over 2400ms = 100ms per slice, 1 call per slice
    let inner: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());
    let config = QuotaConfig {
        limit: 24,
        window: Duration::from_millis(2400),
        strategy: QuotaConsumptionStrategy::EvenSpreadHourly,
    };
    let wrapper = Arc::new(QuotaAwareConnector::new(inner, config));

    // First call establishes initial slice boundary at T=0
    assert!(wrapper.should_allow_call().is_ok());

    // Wait 250ms, crossing 2 complete slice boundaries (slices at 100ms and 200ms)
    std::thread::sleep(Duration::from_millis(250));

    // Second call should reset to the slice starting at T=200ms (not T=250ms)
    assert!(wrapper.should_allow_call().is_ok());

    // Wait 60ms, crossing into the next slice (slice starting at T=300ms)
    std::thread::sleep(Duration::from_millis(60));

    // This call should succeed because we've entered the next 100ms slice window.
    // If slice boundaries drifted, this would fail because we'd still be within
    // the same slice that started at T=250ms (only 60ms elapsed).
    assert!(
        wrapper.should_allow_call().is_ok(),
        "Slice boundaries should remain aligned to regular 100ms intervals"
    );
}
