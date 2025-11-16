use crate::helpers::{MSFT, MockConnector};
use borsa::Borsa;
use borsa_core::{AssetKind, Decimal, MajorHolder};

#[tokio::test]
async fn holders_falls_back_and_succeeds() {
    let err = MockConnector::builder()
        .name("err_holders")
        .with_major_holders_fn(|_i| Err(borsa_core::BorsaError::unsupported("major_holders")))
        .build();
    let ok = MockConnector::builder()
        .name("ok_holders")
        .returns_major_holders_ok(vec![MajorHolder {
            category: "Test".into(),
            value: dec("0.0"),
        }])
        .build();

    let borsa = Borsa::builder()
        .with_connector(err)
        .with_connector(ok)
        .build()
        .unwrap();

    let inst = crate::helpers::instrument(&MSFT, AssetKind::Equity);
    let rows = borsa.major_holders(&inst).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].value, dec("0.0"));
}

fn dec(input: &str) -> Decimal {
    input.parse().expect("valid decimal literal")
}
