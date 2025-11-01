use crate::helpers::usd;
use crate::helpers::{AAPL, MockConnector};
use borsa::Borsa;
use borsa_core::{AssetKind, BorsaConnector, PriceTarget, RoutingPolicyBuilder};
use rust_decimal::Decimal;
use tokio::time::Duration;

#[tokio::test]
async fn price_target_respects_per_kind_priority() {
    let low_pt = PriceTarget {
        mean: Some(usd("180.0")),
        high: Some(usd("200.0")),
        low: Some(usd("150.0")),
        number_of_analysts: Some(10),
    };
    let high_pt = PriceTarget {
        mean: Some(usd("210.0")),
        high: Some(usd("250.0")),
        low: Some(usd("180.0")),
        number_of_analysts: Some(42),
    };

    let low_arc = MockConnector::builder()
        .name("low")
        .returns_analyst_price_target_ok(low_pt)
        .build();
    let high_arc = MockConnector::builder()
        .name("high")
        .delay(Duration::from_millis(80))
        .returns_analyst_price_target_ok(high_pt)
        .build();

    let policy = RoutingPolicyBuilder::new()
        .providers_for_kind(AssetKind::Equity, &[high_arc.key(), low_arc.key()])
        .build();
    let borsa = Borsa::builder()
        .with_connector(low_arc.clone())
        .with_connector(high_arc.clone())
        .routing_policy(policy)
        .build()
        .unwrap();

    let inst = crate::helpers::instrument(&AAPL, AssetKind::Equity);
    let out = borsa.analyst_price_target(&inst).await.unwrap();
    assert_eq!(out.high.map(|m| m.amount()), Some(Decimal::from(250u8)));
    assert_eq!(out.number_of_analysts, Some(42));
}
