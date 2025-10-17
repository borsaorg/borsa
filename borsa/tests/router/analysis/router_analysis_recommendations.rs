use crate::helpers::MockConnector;
use borsa::Borsa;
use borsa_core::{AssetKind, RecommendationRow};

#[tokio::test]
async fn recommendations_respects_per_kind_priority() {
    let low_arc = MockConnector::builder()
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
    let high_arc = MockConnector::builder()
        .name("high")
        .delay(std::time::Duration::from_millis(80))
        .returns_recommendations_ok(vec![RecommendationRow {
            period: "2024-08".parse().unwrap(),
            strong_buy: Some(5),
            buy: Some(10),
            hold: Some(7),
            sell: Some(1),
            strong_sell: Some(0),
        }])
        .build();

    let borsa = Borsa::builder()
        .with_connector(low_arc.clone())
        .with_connector(high_arc.clone())
        .prefer_for_kind(AssetKind::Equity, &[high_arc, low_arc])
        .build()
        .unwrap();

    let inst = crate::helpers::instrument("AAPL", AssetKind::Equity);
    let out = borsa.recommendations(&inst).await.unwrap();
    // Should come from "high" despite being slower
    assert_eq!(out[0].period, "2024-08".parse().unwrap());
}
