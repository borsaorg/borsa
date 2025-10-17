use borsa::Borsa;

use borsa_core::{AssetKind, HistoryRequest, HistoryResponse};

use crate::helpers::candle;
use crate::helpers::mock_connector::MockConnector;

#[tokio::test]
async fn history_falls_back_when_first_errors() {
    // First connector advertises HISTORY but will error (history: None)
    let first = MockConnector::builder().name("err").build();

    // Second connector succeeds
    let second = MockConnector::builder()
        .name("ok")
        .returns_history_ok(HistoryResponse {
            candles: vec![candle(10, 10.0), candle(11, 11.0)],
            actions: vec![],
            adjusted: false,
            meta: None,
        })
        .build();

    let borsa = Borsa::builder()
        .with_connector(first)
        .with_connector(second)
        .build();

    let inst = crate::helpers::instrument("BTC-USD", AssetKind::Crypto);
    let out = borsa
        .history(
            &inst,
            HistoryRequest::try_from_range(borsa_core::Range::D1, borsa_core::Interval::D1)
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(out.candles.len(), 2);
    assert_eq!(out.candles[0].ts.timestamp(), 10);
}
