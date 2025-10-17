use borsa::{Borsa, FetchStrategy};
use borsa_core::{AssetKind, BorsaError};

#[tokio::test]
async fn quote_returns_unsupported_when_no_connector_handles_capability() {
    let borsa = Borsa::builder().build();

    let inst = crate::helpers::instrument("X", AssetKind::Equity);
    let err = borsa.quote(&inst).await.unwrap_err();
    match err {
        BorsaError::Unsupported { capability } => assert_eq!(capability, "quote"),
        other => panic!("expected Unsupported error, got {other:?}"),
    }
}

#[tokio::test]
async fn quote_latency_strategy_still_signals_unsupported() {
    let borsa = Borsa::builder()
        .fetch_strategy(FetchStrategy::Latency)
        .build();

    let inst = crate::helpers::instrument("X", AssetKind::Equity);
    let err = borsa.quote(&inst).await.unwrap_err();
    match err {
        BorsaError::Unsupported { capability } => assert_eq!(capability, "quote"),
        other => panic!("expected Unsupported error, got {other:?}"),
    }
}
