use borsa::Borsa;

use crate::helpers::mock_connector::MockConnector;
use crate::helpers::usd;
use borsa_core::{AssetKind, Quote, Symbol};
use rust_decimal::Decimal;

#[tokio::test]
async fn per_kind_priority_applies_to_quotes() {
    let low = MockConnector::builder()
        .name("low")
        .returns_quote_ok(Quote {
            symbol: Symbol::new("X").unwrap(),
            shortname: None,
            price: Some(usd("10.0")),
            previous_close: None,
            exchange: None,
            market_state: None,
        })
        .build();

    let high = MockConnector::builder()
        .name("high")
        .returns_quote_ok(Quote {
            symbol: Symbol::new("X").unwrap(),
            shortname: None,
            price: Some(usd("99.0")),
            previous_close: None,
            exchange: None,
            market_state: None,
        })
        .build();

    let borsa = Borsa::builder()
        .with_connector(low.clone())
        .with_connector(high.clone())
        .prefer_for_kind(AssetKind::Equity, &[high, low])
        .build()
        .unwrap();

    let inst = crate::helpers::instrument("X", AssetKind::Equity);
    let q = borsa.quote(&inst).await.unwrap();
    assert_eq!(
        q.price.as_ref().map(borsa_core::Money::amount).unwrap(),
        Decimal::from(99u8)
    );
}
