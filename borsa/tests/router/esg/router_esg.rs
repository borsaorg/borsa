use crate::helpers::MockConnector;
use borsa::Borsa;
use borsa_core::{AssetKind, EsgScores};

#[tokio::test]
async fn esg_succeeds() {
    let ok = MockConnector::builder()
        .name("ok_esg")
        .returns_esg_ok(EsgScores {
            environmental: Some(10.0),
            social: Some(10.0),
            governance: Some(5.0),
        })
        .build();
    let borsa = Borsa::builder().with_connector(ok).build();

    let inst = crate::helpers::instrument("MSFT", AssetKind::Equity);
    let scores = borsa.sustainability(&inst).await.unwrap();
    assert_eq!(scores.environmental, Some(10.0));
}
