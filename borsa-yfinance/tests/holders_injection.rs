#![cfg(feature = "test-adapters")]

use borsa_core::{
    AssetKind, Currency, Decimal, Instrument, Money,
    connector::{InstitutionalHoldersProvider, MajorHoldersProvider},
};
use borsa_yfinance::{YfConnector, adapter};
use chrono::TimeZone;

use std::sync::Arc;
use yfinance_rs::holders::{InstitutionalHolder, MajorHolder};

struct Combo {
    h: Arc<dyn adapter::YfHolders>,
}

impl adapter::CloneArcAdapters for Combo {
    fn clone_arc_holders(&self) -> Arc<dyn adapter::YfHolders> {
        self.h.clone()
    }
}

#[tokio::test]
async fn holders_uses_injected_adapter_and_maps() {
    let holders_adapter = <dyn adapter::YfHolders>::from_fns(
        |sym| {
            assert_eq!(sym, "MSFT");
            Ok(vec![MajorHolder {
                category: "Test".into(),
                value: dec("0.10"),
            }])
        },
        |sym| {
            assert_eq!(sym, "MSFT");
            Ok(vec![InstitutionalHolder {
                holder: "Vanguard".into(),
                shares: Some(100),
                date_reported: chrono::Utc.timestamp_opt(1, 0).unwrap(),
                pct_held: Some(dec("0.1")),
                value: Some(
                    Money::from_canonical_str("1000", Currency::Iso(borsa_core::IsoCurrency::USD))
                        .unwrap(),
                ),
            }])
        },
        |_| Ok(vec![]),
        |_| Ok(vec![]),
        |_| Ok(vec![]),
        |_| Ok(None),
    );
    let yf = YfConnector::from_adapter(&Combo { h: holders_adapter });
    let inst = Instrument::from_symbol("MSFT", AssetKind::Equity).expect("valid test instrument");

    let major = yf.major_holders(&inst).await.unwrap();
    assert_eq!(major.len(), 1);
    assert_eq!(major[0].category, "Test");
    // value is numeric fraction now (10% -> 0.10)
    assert_eq!(major[0].value, dec("0.10"));

    let institutional = yf.institutional_holders(&inst).await.unwrap();
    assert_eq!(institutional.len(), 1);
    assert_eq!(institutional[0].holder, "Vanguard");
}

fn dec(input: &str) -> Decimal {
    input.parse().expect("valid decimal literal")
}
