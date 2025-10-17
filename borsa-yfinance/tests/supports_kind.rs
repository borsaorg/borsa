use borsa_core::AssetKind;
use borsa_core::BorsaConnector;
use borsa_yfinance::YfConnector;

#[test]
fn yf_connector_kind_matrix() {
    let yf = YfConnector::from_adapter(&borsa_yfinance::adapter::RealAdapter::new_default());
    assert!(yf.supports_kind(AssetKind::Equity));
    assert!(yf.supports_kind(AssetKind::Fund));
    assert!(yf.supports_kind(AssetKind::Index));
    assert!(yf.supports_kind(AssetKind::Crypto));
    assert!(yf.supports_kind(AssetKind::Forex));

    assert!(!yf.supports_kind(AssetKind::Bond));
    assert!(!yf.supports_kind(AssetKind::Commodity));
}
