use borsa_types::{ConnectorKey, RoutingContext, RoutingPolicy, RoutingPolicyBuilder};
use paft::domain::{AssetKind, Exchange};

fn ex(name: &str) -> Exchange {
    Exchange::try_from_str(name).unwrap()
}

fn policy_fixture() -> RoutingPolicy {
    let fast = ConnectorKey::new("fast");
    let slow = ConnectorKey::new("slow");

    RoutingPolicyBuilder::new()
        .providers_for_kind(AssetKind::Equity, &[slow.clone(), fast.clone()])
        .providers_for_symbol("AAPL", &[fast, slow.clone()])
        .providers_rule(
            borsa_types::routing_policy::Selector {
                symbol: None,
                kind: Some(AssetKind::Crypto),
                exchange: None,
            },
            &[slow],
            true,
        )
        .exchanges_global(&[ex("NASDAQ"), ex("NYSE")])
        .exchanges_for_kind(AssetKind::Equity, &[ex("LSE"), ex("NYSE")])
        .exchanges_for_symbol("RIO", &[ex("LSE")])
        .build()
}

#[test]
fn routing_policy_roundtrip_preserves_behavior() {
    let fast = ConnectorKey::new("fast");
    let slow = ConnectorKey::new("slow");

    let policy = policy_fixture();
    let json = serde_json::to_string(&policy).expect("serialize policy");
    let de: RoutingPolicy = serde_json::from_str(&json).expect("deserialize policy");

    // 1) Symbol override (AAPL) prefers fast over slow
    let ctx_aapl = RoutingContext::new(Some("AAPL"), Some(AssetKind::Equity), None);
    let (r_fast, _) = de
        .providers
        .provider_rank(&ctx_aapl, &fast)
        .expect("fast eligible");
    let (r_slow, _) = de
        .providers
        .provider_rank(&ctx_aapl, &slow)
        .expect("slow eligible");
    assert!(
        r_fast < r_slow,
        "symbol override should prefer fast over slow"
    );

    // 2) Kind-level rule (MSFT equity) prefers slow over fast
    let ctx_msft = RoutingContext::new(Some("MSFT"), Some(AssetKind::Equity), None);
    let (r_slow2, _) = de
        .providers
        .provider_rank(&ctx_msft, &slow)
        .expect("slow eligible");
    let (r_fast2, _) = de
        .providers
        .provider_rank(&ctx_msft, &fast)
        .expect("fast eligible");
    assert!(r_slow2 < r_fast2, "kind rule should prefer slow over fast");

    // 3) Strict rule for Crypto: only slow is eligible
    let ctx_btc = RoutingContext::new(Some("BTC-USD"), Some(AssetKind::Crypto), None);
    assert!(de.providers.provider_rank(&ctx_btc, &slow).is_some());
    assert!(de.providers.provider_rank(&ctx_btc, &fast).is_none());

    // 4) Exchange preference for symbol RIO: LSE ranks over NYSE
    let ctx_rio = RoutingContext::new(Some("RIO"), Some(AssetKind::Equity), None);
    let lse = ex("LSE");
    let nyse = ex("NYSE");
    let k_lse = de.exchange_sort_key(&ctx_rio, Some(&lse), 0).0;
    let k_nyse = de.exchange_sort_key(&ctx_rio, Some(&nyse), 1).0;
    assert!(
        k_lse < k_nyse,
        "symbol exchange preference should favor LSE for RIO"
    );

    // 5) Exchange preference for kind-only (DUAL): LSE ranks over NYSE
    let ctx_dual = RoutingContext::new(Some("DUAL"), Some(AssetKind::Equity), None);
    let k_lse2 = de.exchange_sort_key(&ctx_dual, Some(&lse), 0).0;
    let k_nyse2 = de.exchange_sort_key(&ctx_dual, Some(&nyse), 1).0;
    assert!(
        k_lse2 < k_nyse2,
        "kind exchange preference should favor LSE for equities"
    );
}

#[test]
fn precedence_prefers_more_fields_over_symbol_only() {
    use borsa_types::{ConnectorKey, RoutingContext, RoutingPolicyBuilder};
    use paft::domain::{AssetKind, Exchange};

    let slow = ConnectorKey::new("slow");
    let fast = ConnectorKey::new("fast");
    let nyse = Exchange::try_from_str("NYSE").unwrap();

    // Build policy with a kind+exchange rule and a symbol-only rule; both allow fallback.
    let policy = RoutingPolicyBuilder::new()
        .providers_rule(
            borsa_types::routing_policy::Selector {
                symbol: None,
                kind: Some(AssetKind::Equity),
                exchange: Some(nyse.clone()),
            },
            &[slow.clone(), fast.clone()],
            false,
        )
        .providers_for_symbol("AAPL", &[fast.clone(), slow.clone()])
        .build();

    // Context matches both rules: symbol=AAPL, kind=Equity, exchange=NYSE.
    // With count-first precedence, kind+exchange (2 fields) beats symbol-only (1 field).
    let ctx = RoutingContext::new(Some("AAPL"), Some(AssetKind::Equity), Some(nyse));

    let (r_slow, _) = policy
        .providers
        .provider_rank(&ctx, &slow)
        .expect("slow eligible");
    let (r_fast, _) = policy
        .providers
        .provider_rank(&ctx, &fast)
        .expect("fast eligible");

    // The kind+exchange rule lists slow before fast; it should win over the symbol-only rule.
    assert!(
        r_slow < r_fast,
        "kind+exchange rule should take precedence over symbol-only"
    );
}

#[test]
fn borsa_config_roundtrip_serde() {
    use borsa_types::{BackoffConfig, BorsaConfig, FetchStrategy, MergeStrategy, Resampling};
    let cfg = BorsaConfig {
        routing_policy: policy_fixture(),
        prefer_adjusted_history: true,
        resampling: Resampling::Weekly,
        auto_resample_subdaily_to_daily: true,
        fetch_strategy: FetchStrategy::Latency,
        merge_history_strategy: MergeStrategy::Fallback,
        provider_timeout: std::time::Duration::from_secs(7),
        request_timeout: Some(std::time::Duration::from_millis(1500)),
        backoff: Some(BackoffConfig {
            min_backoff_ms: 10,
            max_backoff_ms: 10_000,
            factor: 3,
            jitter_percent: 25,
        }),
    };

    let json = serde_json::to_string(&cfg).expect("serialize cfg");
    let de: BorsaConfig = serde_json::from_str(&json).expect("deserialize cfg");

    assert!(de.prefer_adjusted_history);
    assert_eq!(de.resampling, Resampling::Weekly);
    assert!(de.auto_resample_subdaily_to_daily);
    assert_eq!(de.fetch_strategy, FetchStrategy::Latency);
    assert_eq!(de.merge_history_strategy, MergeStrategy::Fallback);
    assert_eq!(de.provider_timeout.as_secs(), 7);
    assert_eq!(de.request_timeout.unwrap().as_millis(), 1500);

    // Sanity-check provider behavior survives roundtrip
    let fast = ConnectorKey::new("fast");
    let slow = ConnectorKey::new("slow");
    let ctx_aapl = RoutingContext::new(Some("AAPL"), Some(AssetKind::Equity), None);
    let (r_fast, _) = de
        .routing_policy
        .providers
        .provider_rank(&ctx_aapl, &fast)
        .unwrap();
    let (r_slow, _) = de
        .routing_policy
        .providers
        .provider_rank(&ctx_aapl, &slow)
        .unwrap();
    assert!(r_fast < r_slow);
}

#[test]
fn connector_key_roundtrip() {
    let k = ConnectorKey::new("alpha_vantage");
    let json = serde_json::to_string(&k).unwrap();
    let de: ConnectorKey = serde_json::from_str(&json).unwrap();
    assert_eq!(de.as_str(), "alpha_vantage");
}
