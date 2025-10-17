use crate::helpers::MockConnector;
use borsa::Borsa;
use borsa_core::AssetKind;

#[tokio::test]
async fn expirations_falls_back_and_succeeds() {
    let err = MockConnector::builder()
        .name("err_exp")
        .with_options_expirations_fn(|_i| {
            Err(borsa_core::BorsaError::unsupported("options/expirations"))
        })
        .build();
    let ok = MockConnector::builder()
        .name("ok_exp")
        .returns_options_expirations_ok(vec![1_725_813_600, 1_726_400_000])
        .build();

    let borsa = Borsa::builder()
        .with_connector(err)
        .with_connector(ok)
        .build();

    let inst = crate::helpers::instrument("AAPL", AssetKind::Equity);
    let out = borsa.options_expirations(&inst).await.unwrap();
    assert_eq!(out.len(), 2);
    assert_eq!(out[0], 1_725_813_600);
}
