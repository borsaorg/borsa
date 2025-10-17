#![cfg(feature = "test-adapters")]

use std::sync::Arc;

use borsa_core::{AssetKind, CompanyProfile, Instrument, Isin, connector::ProfileProvider};
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
async fn profile_uses_injected_adapter() {
    let prof = <dyn adapter::YfProfile>::from_fn(|symbol| {
        assert_eq!(symbol, "MSFT");
        Ok(yfinance_rs::profile::Profile::Company(CompanyProfile {
            name: "Microsoft Corporation".into(),
            sector: Some("Technology".into()),
            industry: Some("Software".into()),
            website: Some("https://microsoft.com".into()),
            summary: None,
            address: None,
            isin: Some(Isin::new("US5949181045").unwrap()),
        }))
    });

    let yf = YfConnector::from_adapter(&Combo { p: prof });

    let inst = Instrument::from_symbol("MSFT", AssetKind::Equity).expect("valid test instrument");
    let p = yf.profile(&inst).await.unwrap();
    match p {
        borsa_core::Profile::Company(c) => {
            assert_eq!(c.name, "Microsoft Corporation");
            assert_eq!(c.sector.as_deref(), Some("Technology"));
            assert_eq!(c.industry.as_deref(), Some("Software"));
        }
        borsa_core::Profile::Fund(_) => panic!("expected company profile"),
    }
}
