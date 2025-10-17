use borsa::Borsa;

use borsa_core::{AssetKind, HistoryRequest, HistoryResponse};

use crate::helpers::candle;
use crate::helpers::mock_connector::MockConnector;

#[tokio::test]
async fn empty_history_result_is_skipped() {
    // First returns Ok with empty candles (should be ignored)
    let empty = MockConnector::builder()
        .name("empty")
        .returns_history_ok(HistoryResponse {
            candles: vec![],
            actions: vec![],
            adjusted: false,
            meta: None,
        })
        .build();

    // Second returns data
    let filled = MockConnector::builder()
        .name("filled")
        .returns_history_ok(HistoryResponse {
            candles: vec![candle(1, 1.0), candle(2, 2.0)],
            actions: vec![],
            adjusted: false,
            meta: None,
        })
        .build();

    let borsa = Borsa::builder()
        .with_connector(empty)
        .with_connector(filled)
        .build()
        .unwrap();

    let inst = crate::helpers::instrument("ETH-USD", AssetKind::Crypto);
    let out = borsa
        .history(
            &inst,
            HistoryRequest::try_from_range(borsa_core::Range::D1, borsa_core::Interval::D1)
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(out.candles.len(), 2);
    assert_eq!(out.candles[0].ts.timestamp(), 1);
}
