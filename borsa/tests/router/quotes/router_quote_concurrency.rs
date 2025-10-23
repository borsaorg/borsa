use borsa::Borsa;
use borsa_core::{AssetKind, BorsaConnector, Quote, RoutingPolicyBuilder, Symbol};

use crate::helpers::{m_quote, mock_connector::MockConnector, usd};
use std::time::Duration;

#[tokio::test]
async fn faster_lower_priority_does_not_beat_higher_priority_success() {
    let low = m_quote("low", 10.0);
    let high = MockConnector::builder()
        .name("high")
        .delay(Duration::from_millis(80))
        .returns_quote_ok(Quote {
            symbol: Symbol::new("X").unwrap(),
            shortname: None,
            price: Some(usd("99.0")),
            previous_close: None,
            exchange: None,
            market_state: None,
        })
        .build();

    let policy = RoutingPolicyBuilder::new()
        .providers_for_symbol("X", &[high.key(), low.key()])
        .build();

    let borsa = Borsa::builder()
        .with_connector(low.clone())
        .with_connector(high.clone())
        .routing_policy(policy)
        .build()
        .unwrap();

    let inst = crate::helpers::instrument("X", AssetKind::Equity);
    let q = borsa.quote(&inst).await.unwrap();
    assert_eq!(q.price.unwrap().amount().to_string(), "99.0");
}
