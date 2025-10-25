use std::sync::Arc;
use std::time::Duration;

use borsa_core::{AssetKind, BorsaConnector, BorsaError, Instrument};
use borsa_middleware::{BlacklistingMiddleware, QuotaAwareConnector};
use borsa_mock::MockConnector;
use borsa_types::{QuotaConfig, QuotaConsumptionStrategy, QuotaState};

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
    let st = QuotaState {
        limit: cfg.limit,
        remaining: cfg.limit,
        reset_in: cfg.window,
    };
    Arc::new(QuotaAwareConnector::new(inner, cfg, st))
}

#[tokio::test]
async fn daily_quota_blacklists_provider() {
    let quota = make_quota_wrapper(1, 86_400_000, QuotaConsumptionStrategy::Unit);
    let wrapped: Arc<dyn BorsaConnector> = Arc::new(BlacklistingMiddleware::new(
        quota as Arc<dyn BorsaConnector>,
        Duration::from_secs(24 * 60 * 60),
    ));
    let qp = wrapped
        .as_quote_provider()
        .expect("quote capability present");

    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid symbol");

    // First call succeeds
    let _ = qp.quote(&inst).await.expect("first call ok");
    // Second call exceeds quota (returns QuotaExceeded and triggers blacklist)
    let err = qp.quote(&inst).await.expect_err("second call should fail");
    assert!(matches!(err, BorsaError::QuotaExceeded { .. }));
    // Third call should be immediately blocked by middleware blacklist
    let err2 = qp
        .quote(&inst)
        .await
        .expect_err("third call should be blacklisted");
    assert!(matches!(err2, BorsaError::TemporarilyBlacklisted { .. }));
}

#[tokio::test]
async fn temporary_quota_spread_does_not_blacklist_long_term() {
    // Use a small window to make slice short for the test (e.g., 2400ms -> 100ms slices)
    let quota = make_quota_wrapper(24, 2400, QuotaConsumptionStrategy::EvenSpreadHourly);
    let wrapped: Arc<dyn BorsaConnector> = Arc::new(BlacklistingMiddleware::new(
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
    // Immediate next call blacklisted by middleware for ~slice duration
    let err2 = qp
        .quote(&inst)
        .await
        .expect_err("third call should be blacklisted");
    assert!(matches!(err2, BorsaError::TemporarilyBlacklisted { .. }));
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
    let st = QuotaState {
        limit: cfg.limit,
        remaining: cfg.limit,
        reset_in: cfg.window,
    };
    let quota = Arc::new(QuotaAwareConnector::new(inner, cfg, st));
    let wrapped: Arc<dyn BorsaConnector> = Arc::new(BlacklistingMiddleware::new(
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
    let wrapped: Arc<dyn BorsaConnector> = Arc::new(BlacklistingMiddleware::new(
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
