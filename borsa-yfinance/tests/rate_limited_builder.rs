use borsa_yfinance::YfConnector;

#[test]
fn rate_limited_builder_produces_expected_stack_and_name() {
    let builder = YfConnector::rate_limited();
    let stack = builder.to_stack();

    // Expect at least Blacklist, Quota and Raw layers in some order (outer->inner)
    assert!(
        stack
            .layers
            .iter()
            .any(|l| l.name == "BlacklistConnector")
    );
    assert!(stack.layers.iter().any(|l| l.name == "QuotaAwareConnector"));
    assert!(stack.layers.iter().any(|l| l.name == "RawConnector"));

    // Build and confirm identity is preserved as borsa-yfinance
    let wrapped = builder
        .build()
        .expect("middleware validation should succeed");
    assert_eq!(wrapped.name(), "borsa-yfinance");
}
