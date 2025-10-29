use std::sync::Arc;

use borsa_core::BorsaConnector;
use borsa_middleware::ConnectorBuilder;
use borsa_mock::MockConnector;
use borsa_types::{CacheConfig, QuotaConfig, QuotaConsumptionStrategy};

const fn default_quota() -> QuotaConfig {
    QuotaConfig { limit: 10, window: std::time::Duration::from_secs(60), strategy: QuotaConsumptionStrategy::Unit }
}

#[tokio::test]
async fn builder_ordering_policy_is_enforced() {
    let raw: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());
    let mut cfg = CacheConfig::default();
    cfg.per_capability_ttl_ms.insert("quote".into(), 1000);
    let b = ConnectorBuilder::new(raw)
        .with_quota(&default_quota())
        .with_blacklist(std::time::Duration::from_secs(300))
        .with_cache(&cfg);
    let stack = b.to_stack();
    let names: Vec<_> = stack.layers.iter().map(|l| l.name.as_str()).collect();
    assert!(names.len() >= 3);
    assert_eq!(names[0], "CachingMiddleware");
    assert_eq!(names[1], "BlacklistingMiddleware");
    assert_eq!(names[2], "QuotaAwareConnector");
}



