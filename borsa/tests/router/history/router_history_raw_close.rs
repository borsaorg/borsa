use borsa::{Borsa, Resampling};

use crate::helpers::usd;
use borsa_core::{AssetKind, Candle, HistoryRequest, HistoryResponse};
use chrono::TimeZone;

use crate::helpers::mock_connector::MockConnector;

#[tokio::test]
async fn raw_close_preserved_for_single_source_no_resample() {
    let hist = HistoryResponse {
        candles: vec![
            Candle {
                ts: chrono::Utc.timestamp_opt(1, 0).unwrap(),
                open: usd("1.0"),
                high: usd("1.0"),
                low: usd("1.0"),
                close: usd("1.0"),
                close_unadj: Some(usd("1.0")),
                volume: None,
            },
            Candle {
                ts: chrono::Utc.timestamp_opt(2, 0).unwrap(),
                open: usd("2.0"),
                high: usd("2.0"),
                low: usd("2.0"),
                close: usd("2.0"),
                close_unadj: Some(usd("2.0")),
                volume: None,
            },
        ],
        actions: vec![],
        adjusted: true,
        meta: None,
    };

    let c = MockConnector::builder()
        .name("single")
        .returns_history_ok(hist)
        .build();

    let borsa = Borsa::builder().with_connector(c).build().unwrap();

    let inst = crate::helpers::instrument("AAPL", AssetKind::Equity);
    let out = borsa
        .history(
            &inst,
            HistoryRequest::try_from_range(borsa_core::Range::M1, borsa_core::Interval::D1)
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(out.candles.len(), 2);
    assert_eq!(
        out.candles
            .iter()
            .map(|c| c.close_unadj.as_ref().map(|m| m.amount().to_string()))
            .collect::<Vec<_>>(),
        vec![Some("1.0".into()), Some("2.0".into())]
    );
}

#[tokio::test]
async fn raw_close_dropped_when_resampled() {
    let hist = HistoryResponse {
        candles: vec![
            // Intraday-like timestamps; triggers resample
            Candle {
                ts: chrono::Utc.timestamp_opt(10, 0).unwrap(),
                open: usd("1.0"),
                high: usd("1.0"),
                low: usd("1.0"),
                close: usd("1.0"),
                close_unadj: Some(usd("1.0")),
                volume: None,
            },
            Candle {
                ts: chrono::Utc.timestamp_opt(20, 0).unwrap(),
                open: usd("2.0"),
                high: usd("2.0"),
                low: usd("2.0"),
                close: usd("2.0"),
                close_unadj: Some(usd("2.0")),
                volume: None,
            },
            Candle {
                ts: chrono::Utc.timestamp_opt(86_410, 0).unwrap(),
                open: usd("3.0"),
                high: usd("3.0"),
                low: usd("3.0"),
                close: usd("3.0"),
                close_unadj: Some(usd("3.0")),
                volume: None,
            },
        ],
        actions: vec![],
        adjusted: true,
        meta: None,
    };

    let c = MockConnector::builder()
        .name("single")
        .returns_history_ok(hist)
        .build();

    let borsa = Borsa::builder()
        .with_connector(c)
        .resampling(Resampling::Daily)
        .build()
        .unwrap();

    let inst = crate::helpers::instrument("BTC-USD", AssetKind::Crypto);
    let out = borsa
        .history(
            &inst,
            HistoryRequest::try_from_range(borsa_core::Range::M1, borsa_core::Interval::D1)
                .unwrap(),
        )
        .await
        .unwrap();

    // Resampled -> daily candles; per-candle close_unadj must be dropped.
    assert!(out.candles.iter().all(|c| c.close_unadj.is_none()));
}
