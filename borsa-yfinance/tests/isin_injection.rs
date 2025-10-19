#![cfg(feature = "test-adapters")]

use std::sync::Arc;

use borsa_core::connector::IsinProvider;
use borsa_core::{AssetKind, Instrument, Isin};
use borsa_yfinance::{YfConnector, adapter};

struct Combo {
    p: Arc<dyn adapter::YfProfile>,
}

impl adapter::CloneArcAdapters for Combo {
    fn clone_arc_profile(&self) -> Arc<dyn adapter::YfProfile> {
        self.p.clone()
    }
}

#[tokio::test]
async fn isin_uses_injected_profile_adapter() {
    let prof = <dyn adapter::YfProfile>::from_fns(
        |_| Err(borsa_core::BorsaError::unsupported("profile")),
        |symbol| {
            assert_eq!(symbol, "AAPL");
            Ok(Some("US0378331005".to_string()))
        },
    );
    let yf = YfConnector::from_adapter(&Combo { p: prof });

    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).unwrap();
    let got = IsinProvider::isin(&yf, &inst).await.unwrap();
    assert_eq!(got, Some(Isin::new("US0378331005").unwrap()));
}
