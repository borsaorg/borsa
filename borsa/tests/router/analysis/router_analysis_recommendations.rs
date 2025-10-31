use crate::helpers::{AAPL, MockConnector};
use borsa::Borsa;
use borsa_core::{AssetKind, BorsaConnector, RecommendationRow, RoutingPolicyBuilder};
use tokio::time::Duration;

#[tokio::test]
async fn recommendations_respects_per_kind_priority() {
    let low = MockConnector::builder()
        .name("low")
        .returns_recommendations_ok(vec![RecommendationRow {
            period: "2024-07".parse().unwrap(),
            strong_buy: Some(1),
            buy: Some(1),
            hold: Some(1),
            sell: Some(1),
            strong_sell: Some(1),
        }])
        .build();
    let high = MockConnector::builder()
        .name("high")
        .delay(Duration::from_millis(80))
        .returns_recommendations_ok(vec![RecommendationRow {
            period: "2024-08".parse().unwrap(),
            strong_buy: Some(5),
            buy: Some(10),
            hold: Some(7),
            sell: Some(1),
            strong_sell: Some(0),
        }])
        .build();

    let policy = RoutingPolicyBuilder::new()
        .providers_for_kind(AssetKind::Equity, &[high.key(), low.key()])
        .build();

    let borsa = Borsa::builder()
        .with_connector(low.clone())
        .with_connector(high.clone())
        .routing_policy(policy)
        .build()
        .unwrap();
    
    let inst = crate::helpers::instrument(&AAPL, AssetKind::Equity);
    let out = borsa.recommendations(&inst).await.unwrap();

    // Should come from "high" despite being slower.
    assert_eq!(out[0].period, "2024-08".parse().unwrap());
}
