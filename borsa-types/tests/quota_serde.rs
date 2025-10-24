use borsa_types::{QuotaConfig, QuotaConsumptionStrategy, QuotaState};

#[test]
fn quota_config_roundtrip() {
    let cfg = QuotaConfig {
        limit: 500,
        window: std::time::Duration::from_secs(120),
        strategy: QuotaConsumptionStrategy::Weighted,
    };

    let json = serde_json::to_string(&cfg).expect("serialize quota config");
    let de: QuotaConfig = serde_json::from_str(&json).expect("deserialize quota config");

    assert_eq!(de.limit, 500);
    assert_eq!(de.window.as_secs(), 120);
    assert!(matches!(de.strategy, QuotaConsumptionStrategy::Weighted));
}

#[test]
fn quota_state_roundtrip() {
    let st = QuotaState {
        limit: 1000,
        remaining: 321,
        reset_in: std::time::Duration::from_millis(8500),
    };

    let json = serde_json::to_string(&st).expect("serialize quota state");
    let de: QuotaState = serde_json::from_str(&json).expect("deserialize quota state");

    assert_eq!(de.limit, 1000);
    assert_eq!(de.remaining, 321);
    assert_eq!(de.reset_in.as_millis(), 8500);
}
