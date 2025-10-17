use crate::helpers::MockConnector;
use borsa::Borsa;
use borsa_core::{
    AssetKind, BorsaError, RecommendationAction, RecommendationGrade, UpgradeDowngradeRow,
};
// use chrono::TimeZone;
use crate::helpers::dt;

#[tokio::test]
async fn upgrades_downgrades_falls_back_and_succeeds() {
    let err = MockConnector::builder()
        .name("err_ud")
        .with_upgrades_downgrades_fn(|_i| {
            Err(BorsaError::unsupported("analysis/upgrades_downgrades"))
        })
        .build();
    let ok = MockConnector::builder()
        .name("ok_ud")
        .returns_upgrades_downgrades_ok(vec![UpgradeDowngradeRow {
            ts: dt(2024, 2, 24, 0, 0, 0),
            firm: Some("ABC".into()),
            from_grade: Some(RecommendationGrade::Hold),
            to_grade: Some(RecommendationGrade::Buy),
            action: Some("up".parse::<RecommendationAction>().unwrap()),
        }])
        .build();

    let borsa = Borsa::builder()
        .with_connector(err)
        .with_connector(ok)
        .build()
        .unwrap();

    let inst = crate::helpers::instrument("MSFT", AssetKind::Equity);
    let rows = borsa.upgrades_downgrades(&inst).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].firm.as_deref(), Some("ABC"));
}

#[tokio::test]
async fn upgrades_downgrades_mapping() -> Result<(), BorsaError> {
    let u = UpgradeDowngradeRow {
        ts: chrono::Utc::now(),
        firm: Some("Firm".into()),
        from_grade: None,
        to_grade: None,
        action: Some("up".parse::<RecommendationAction>().unwrap()),
    };
    assert_eq!(u.action.unwrap().code(), "UPGRADE");
    Ok(())
}
