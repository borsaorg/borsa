use std::sync::Arc;
use std::time::Duration;

use borsa::FetchStrategy;
use borsa::{Borsa, MergeStrategy};
use borsa_core::{AssetKind, BorsaConnector, Instrument};
use borsa_middleware::QuotaAwareConnector;
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
    // limit=1 in a long window simulates daily budget
    let wrapped = make_quota_wrapper(1, 86_400_000, QuotaConsumptionStrategy::Unit);

    let borsa = Borsa::builder()
        .with_connector(wrapped.clone())
        .merge_history_strategy(MergeStrategy::Fallback)
        .fetch_strategy(FetchStrategy::PriorityWithFallback)
        .build()
        .expect("borsa builds");

    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid symbol");

    // First call succeeds and consumes the only unit
    let _ = borsa.quote(&inst).await.expect("first call ok");

    // Second call should not hit the provider (blacklisted after quota exceeded)
    // It should return the provider's QuotaExceeded aggregated as AllProvidersFailed (no other providers)
    let err = borsa
        .quote(&inst)
        .await
        .expect_err("second call should fail");
    let s = err.to_string();
    assert!(s.contains("quota exceeded") || s.contains("all providers failed"));
}

#[tokio::test]
async fn temporary_quota_spread_does_not_blacklist_long_term() {
    // EvenSpreadHourly blocks temporarily within slices but not overall window
    let wrapped = make_quota_wrapper(
        24,
        24 * 60 * 60 * 1000,
        QuotaConsumptionStrategy::EvenSpreadHourly,
    );
    // Fallback provider that always succeeds
    let fallback: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());

    let borsa = Borsa::builder()
        .with_connector(wrapped.clone()) // first, so it is attempted before fallback
        .with_connector(fallback.clone())
        .merge_history_strategy(MergeStrategy::Fallback)
        .fetch_strategy(FetchStrategy::PriorityWithFallback)
        .build()
        .expect("borsa builds");

    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid symbol");

    // Consume one allowed call in the current slice
    let _ = borsa.quote(&inst).await.expect("first call ok");

    // Next call in same slice should hit the spread block on first provider, then fallback succeeds
    let _ = borsa
        .quote(&inst)
        .await
        .expect("fallback should succeed without long-term blacklist");
}

#[tokio::test]
async fn rate_limit_transient_no_blacklist() {
    // Single provider that will report RateLimitExceeded via heuristic
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
    let wrapped = Arc::new(QuotaAwareConnector::new(inner, cfg, st));

    let borsa = Borsa::builder()
        .with_connector(wrapped.clone())
        .merge_history_strategy(MergeStrategy::Fallback)
        .fetch_strategy(FetchStrategy::PriorityWithFallback)
        .build()
        .expect("borsa builds");

    let inst = Instrument::from_symbol("RATELIMIT", AssetKind::Equity).expect("valid symbol");

    // First call should fail with a non-Unsupported error (transient)
    let err1 = borsa.quote(&inst).await.expect_err("should rate limit");
    assert!(
        !err1
            .to_string()
            .to_lowercase()
            .contains("unsupported capability")
    );

    // Second call should also attempt and fail similarly, proving no blacklist
    let err2 = borsa
        .quote(&inst)
        .await
        .expect_err("should still attempt and fail");
    assert!(
        !err2
            .to_string()
            .to_lowercase()
            .contains("unsupported capability")
    );
}

#[tokio::test]
async fn blacklist_expiry_allows_provider_again() {
    // Use a small window to simulate quick expiry
    let wrapped = make_quota_wrapper(1, 50, QuotaConsumptionStrategy::Unit);

    let borsa = Borsa::builder()
        .with_connector(wrapped.clone())
        .merge_history_strategy(MergeStrategy::Fallback)
        .fetch_strategy(FetchStrategy::PriorityWithFallback)
        .build()
        .expect("borsa builds");

    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid symbol");

    // Consume the one unit
    let _ = borsa.quote(&inst).await.expect("first call ok");
    // Next call hits quota and triggers blacklist until reset_in_ms (~50ms)
    let _ = borsa.quote(&inst).await.err();

    // Wait for expiry window
    tokio::time::sleep(Duration::from_millis(60)).await;

    // After expiry, provider should be attempted again and succeed
    let _ = borsa.quote(&inst).await.expect("after expiry ok");
}
