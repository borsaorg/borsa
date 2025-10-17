use borsa::Borsa;
use borsa_core::{AssetKind, BorsaError, HistoryRequest, Interval, Range};

#[tokio::test]
async fn history_reports_unsupported_when_no_history_provider_available() {
    let c = crate::helpers::MockConnector::builder().name("no-history").build();
    let borsa = Borsa::builder().with_connector(c).build().unwrap();
    let inst = crate::helpers::instrument("NOPE", AssetKind::Equity);
    let req = HistoryRequest::try_from_range(Range::D5, Interval::D1).unwrap();

    let err = borsa.history(&inst, req).await.unwrap_err();
    match err {
        BorsaError::Unsupported { capability } => assert_eq!(capability, "history"),
        other => panic!("expected Unsupported error, got {other:?}"),
    }
}
