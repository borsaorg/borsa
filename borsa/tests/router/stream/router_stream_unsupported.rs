use crate::helpers::X;
use borsa::Borsa;
use borsa_core::{AssetKind, BorsaError};

#[tokio::test]
async fn stream_quotes_reports_unsupported_when_no_stream_provider_available() {
    let c = crate::helpers::MockConnector::builder()
        .name("no-stream")
        .build();
    let borsa = Borsa::builder().with_connector(c).build().unwrap();
    let inst = crate::helpers::instrument(&X, AssetKind::Equity);

    let err = borsa.stream_quotes(&[inst]).await.unwrap_err();
    match err {
        BorsaError::Unsupported { capability } => assert_eq!(capability, "stream-quotes"),
        other => panic!("expected Unsupported error, got {other:?}"),
    }
}
