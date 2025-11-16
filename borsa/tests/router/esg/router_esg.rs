use crate::helpers::{MSFT, MockConnector};
use borsa::Borsa;
use borsa_core::{AssetKind, Decimal, EsgScores};

#[tokio::test]
async fn esg_succeeds() {
    let ok = MockConnector::builder()
        .name("ok_esg")
        .returns_esg_ok(EsgScores {
            environmental: Some(dec("10.0")),
            social: Some(dec("10.0")),
            governance: Some(dec("5.0")),
        })
        .build();
    let borsa = Borsa::builder().with_connector(ok).build().unwrap();

    let inst = crate::helpers::instrument(&MSFT, AssetKind::Equity);
    let scores = borsa.sustainability(&inst).await.unwrap();
    assert_eq!(scores.environmental, Some(dec("10.0")));
}

fn dec(input: &str) -> Decimal {
    input.parse().expect("valid decimal literal")
}
