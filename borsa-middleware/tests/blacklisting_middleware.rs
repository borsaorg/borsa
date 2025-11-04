use std::sync::Arc;
use std::time::Duration;

use borsa_core::{AssetKind, BorsaConnector, BorsaError, Instrument};
use borsa_middleware::{BlacklistConnector, QuotaAwareConnector};
use borsa_mock::MockConnector;
use borsa_types::{QuotaConfig, QuotaConsumptionStrategy};

fn make_quota_wrapper(
    limit: u64,
    window_ms: u64,
    strategy: QuotaConsumptionStrategy,
) -> Arc<QuotaAwareConnector> {
    let inner: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());
    let cfg = QuotaConfig {
        limit,
        window: Duration::from_millis(window_ms),
        strategy,
    };
    Arc::new(QuotaAwareConnector::new(inner, cfg))
}

#[tokio::test]
async fn daily_quota_exhaustion_is_bubbled_up() {
    let quota = make_quota_wrapper(1, 86_400_000, QuotaConsumptionStrategy::Unit);
    let wrapped: Arc<dyn BorsaConnector> = Arc::new(BlacklistConnector::new(
        quota as Arc<dyn BorsaConnector>,
        Duration::from_secs(24 * 60 * 60),
    ));
    let qp = wrapped
        .as_quote_provider()
        .expect("quote capability present");

    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid symbol");

    // First call consumes the only remaining daily slot.
    let _ = qp.quote(&inst).await.expect("first call ok");
    // Second call exceeds quota (returns QuotaExceeded and allows middleware to propagate it).
    let err = qp
        .quote(&inst)
        .await
        .expect_err("second call should report quota exhaustion");
    assert!(matches!(err, BorsaError::QuotaExceeded { .. }));
    // Third call still reports QuotaExceeded, confirming the middleware does not blacklist.
    let err2 = qp
        .quote(&inst)
        .await
        .expect_err("third call should still report quota exhaustion");
    assert!(matches!(err2, BorsaError::QuotaExceeded { .. }));
}

#[tokio::test]
async fn temporary_quota_spread_reports_quota_exhaustion() {
    // Use a small window to make slice short for the test (e.g., 2400ms -> 100ms slices)
    let quota = make_quota_wrapper(24, 2400, QuotaConsumptionStrategy::EvenSpreadHourly);
    let wrapped: Arc<dyn BorsaConnector> = Arc::new(BlacklistConnector::new(
        quota as Arc<dyn BorsaConnector>,
        Duration::from_secs(24 * 60 * 60),
    ));
    let qp = wrapped
        .as_quote_provider()
        .expect("quote capability present");

    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid symbol");
    // First call consumes per-slice budget
    let _ = qp.quote(&inst).await.expect("first call ok");
    // Second call should hit slice block -> QuotaExceeded with remaining > 0
    let err = qp
        .quote(&inst)
        .await
        .expect_err("second call should be slice-blocked");
    assert!(matches!(err, BorsaError::QuotaExceeded { .. }));
    // Immediate next call still reports QuotaExceeded; middleware does not blacklist.
    let err2 = qp
        .quote(&inst)
        .await
        .expect_err("third call should still report quota exhaustion");
    assert!(matches!(err2, BorsaError::QuotaExceeded { .. }));
    // Wait beyond slice duration and ensure calls are allowed again
    tokio::time::sleep(Duration::from_millis(150)).await;
    let _ = qp.quote(&inst).await.expect("after slice reset ok");
}

#[tokio::test]
async fn rate_limit_triggers_blacklist() {
    let inner: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());
    let cfg = QuotaConfig {
        limit: 10,
        window: Duration::from_millis(1000),
        strategy: QuotaConsumptionStrategy::Unit,
    };
    let quota = Arc::new(QuotaAwareConnector::new(inner, cfg));
    let wrapped: Arc<dyn BorsaConnector> = Arc::new(BlacklistConnector::new(
        quota as Arc<dyn BorsaConnector>,
        Duration::from_secs(24 * 60 * 60),
    ));
    let qp = wrapped
        .as_quote_provider()
        .expect("quote capability present");

    let inst = Instrument::from_symbol("RATELIMIT", AssetKind::Equity).expect("valid symbol");
    let err1 = qp.quote(&inst).await.expect_err("should rate limit");
    assert!(matches!(err1, BorsaError::RateLimitExceeded { .. }));
    // After a provider rate limit, the middleware should temporarily blacklist the provider.
    let err2 = qp
        .quote(&inst)
        .await
        .expect_err("should be blacklisted after rate limit");
    assert!(matches!(err2, BorsaError::TemporarilyBlacklisted { .. }));
}

#[tokio::test]
async fn blacklist_expiry_allows_provider_again() {
    let quota = make_quota_wrapper(1, 50, QuotaConsumptionStrategy::Unit);
    let wrapped: Arc<dyn BorsaConnector> = Arc::new(BlacklistConnector::new(
        quota as Arc<dyn BorsaConnector>,
        Duration::from_secs(24 * 60 * 60),
    ));
    let qp = wrapped
        .as_quote_provider()
        .expect("quote capability present");

    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid symbol");
    let _ = qp.quote(&inst).await.expect("first call ok");
    let _ = qp.quote(&inst).await.err(); // triggers blacklist
    // Immediate call blacklisted
    let _ = qp.quote(&inst).await.expect_err("should be blacklisted");
    tokio::time::sleep(Duration::from_millis(60)).await;
    let _ = qp.quote(&inst).await.expect("after expiry ok");
}
