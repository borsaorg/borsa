use borsa_core::BorsaConnector;
use borsa_yfinance::YfConnector;

#[test]
fn yf_connector_advertises_profile_capability() {
    let yf = YfConnector::new_default();
    assert!(yf.as_profile_provider().is_some());
}
