use borsa::Borsa;

use crate::helpers::mock_connector::MockConnector;
use crate::helpers::usd;
use borsa_core::{AssetKind, Quote, Symbol};

#[tokio::test]
async fn faster_lower_priority_does_not_beat_higher_priority_success() {
    // Low-priority connector returns quickly with last=1.0
    let low = MockConnector::builder()
        .name("low")
        .returns_quote_ok(Quote {
            symbol: Symbol::new("X").unwrap(),
            shortname: None,
            price: Some(usd("1.0")),
            previous_close: None,
            exchange: None,
            market_state: None,
        })
        .build();

    // High-priority connector returns later with last=99.0
    let high = MockConnector::builder()
        .name("high")
        .delay(std::time::Duration::from_millis(80))
        .returns_quote_ok(Quote {
            symbol: Symbol::new("X").unwrap(),
            shortname: None,
            price: Some(usd("99.0")),
            previous_close: None,
            exchange: None,
            market_state: None,
        })
        .build();

    // Register in any order, then prefer "high" for this symbol.
    let borsa = Borsa::builder()
        .with_connector(low.clone())
        .with_connector(high.clone())
        .prefer_symbol("X", &[high, low])
        .build();

    let inst = crate::helpers::instrument("X", AssetKind::Equity);
    let q = borsa.quote(&inst).await.unwrap();

    // Even though "low" was faster, "high" has higher priority and succeeded,
    // so the router must return 99.0.
    assert_eq!(q.price.unwrap().amount().to_string(), "99.0");
}
