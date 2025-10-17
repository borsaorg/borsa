use borsa_core::AssetKind;

use crate::helpers::MockConnector;

#[tokio::test]
async fn stream_quotes_errors_when_no_stream_connector() {
    let c = MockConnector::builder()
        .name("Q")
        .supports_kind(AssetKind::Equity)
        .build();

    let borsa = borsa::Borsa::builder().with_connector(c).build().unwrap();
    let res = borsa
        .stream_quotes(&[crate::helpers::instrument("AAPL", AssetKind::Equity)])
        .await;
    assert!(res.is_err());
}
