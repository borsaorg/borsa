use borsa::Borsa;
use borsa_core::{AssetKind, BorsaError, SearchRequest};

use crate::helpers::MockConnector;

#[tokio::test]
async fn search_returns_unsupported_when_no_provider_available() {
    let connector = MockConnector::builder()
        .name("no-search")
        .supports_kind(AssetKind::Equity)
        .build();

    let borsa = Borsa::builder().with_connector(connector).build().unwrap();

    let req = SearchRequest::builder("AAPL")
        .kind(AssetKind::Equity)
        .build()
        .unwrap();

    let err = borsa.search(req).await.unwrap_err();
    match err {
        BorsaError::Unsupported { capability } => assert_eq!(capability, "search"),
        other => panic!("expected Unsupported error, got {other:?}"),
    }
}
