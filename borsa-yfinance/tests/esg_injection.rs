#![cfg(feature = "test-adapters")]

use std::sync::Arc;

use borsa_core::{AssetKind, Instrument, connector::EsgProvider};
use borsa_yfinance::{YfConnector, adapter};

struct Combo {
    e: Arc<dyn adapter::YfEsg>,
}
impl adapter::CloneArcAdapters for Combo {
    fn clone_arc_esg(&self) -> Arc<dyn adapter::YfEsg> {
        self.e.clone()
    }
}

#[tokio::test]
async fn esg_injection_maps_correctly() {
    let esg_adapter = <dyn adapter::YfEsg>::from_fn(|sym| {
        assert_eq!(sym, "AAPL");
        Ok(yfinance_rs::esg::EsgScores {
            environmental: Some(1.0),
            social: Some(2.0),
            governance: Some(3.0),
        })
    });

    let yf = YfConnector::from_adapter(&Combo { e: esg_adapter });
    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid test instrument");

    let sc = yf.sustainability(&inst).await.unwrap();
    assert_eq!(sc.environmental, Some(1.0));
}
