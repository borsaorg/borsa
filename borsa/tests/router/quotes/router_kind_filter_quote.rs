use borsa::Borsa;

use crate::helpers::{X, usd};
use borsa_core::{AssetKind, Quote};
use rust_decimal::Decimal;

// Bring in the MockConnector directly to control kind support.
use crate::helpers::mock_connector::MockConnector;

#[tokio::test]
async fn router_skips_connectors_that_do_not_support_kind_for_quote() {
    // Connector A supports only Equity; returns 1.0
    let a = MockConnector::builder()
        .name("A")
        .supports_kind(AssetKind::Equity)
        .returns_quote_ok(Quote {
            symbol: X.clone(),
            shortname: None,
            price: Some(usd("1.0")),
            previous_close: None,
            exchange: None,
            market_state: None,
            day_volume: None,
        })
        .build();

    // Connector B supports only Fund; returns 99.0
    let b = MockConnector::builder()
        .name("B")
        .supports_kind(AssetKind::Fund)
        .returns_quote_ok(Quote {
            symbol: X.clone(),
            shortname: None,
            price: Some(usd("99.0")),
            previous_close: None,
            exchange: None,
            market_state: None,
            day_volume: None,
        })
        .build();

    let borsa = Borsa::builder()
        .with_connector(a)
        .with_connector(b)
        .build()
        .unwrap();
    
    let inst = crate::helpers::instrument(&X, AssetKind::Fund);
    let q = borsa.quote(&inst).await.unwrap();
    assert_eq!(
        q.price.as_ref().map(borsa_core::Money::amount).unwrap(),
        Decimal::from(99u8),
        "should have used connector B that supports Fund"
    );
}
