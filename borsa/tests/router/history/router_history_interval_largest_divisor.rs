use crate::helpers::MockConnector;
use crate::helpers::usd;
use borsa::Borsa;
use borsa_core::{AssetKind, Candle, HistoryRequest, HistoryResponse, Interval};
use chrono::TimeZone;
use rust_decimal::Decimal;
use tokio::test;

const fn supported_30m_60m() -> &'static [borsa_core::Interval] {
    &[borsa_core::Interval::I30m, borsa_core::Interval::I1h]
}

fn c(ts: i64, close: f64) -> Candle {
    let ts = chrono::Utc.timestamp_opt(ts, 0).unwrap();
    let m = usd(&close.to_string());
    Candle {
        ts,
        open: m.clone(),
        high: m.clone(),
        low: m.clone(),
        close: m,
        close_unadj: None,
        volume: None,
    }
}

#[test]
async fn picks_largest_divisor_and_resamples_to_requested() {
    let c = MockConnector::builder()
        .name("interval-aware")
        .with_history_intervals(supported_30m_60m())
        .with_history_fn(|_i, _r| {
            // Provide 30m-like data; behavior assertions will validate resampling outcome
            let candles: Vec<Candle> = vec![c(0, 1.0), c(1800, 2.0), c(3600, 3.0), c(5400, 4.0)];
            Ok(HistoryResponse {
                candles,
                actions: vec![],
                adjusted: false,
                meta: None,
            })
        })
        .build();
    let borsa = Borsa::builder().with_connector(c).build();
    let inst = crate::helpers::instrument("ETH-USD", AssetKind::Crypto);

    // Request 90-minute bars
    let req = HistoryRequest::try_from_range(borsa_core::Range::D1, Interval::I90m).unwrap();

    // Router should pick 30m (since 90 % 30 == 0, and 30m > any other valid divisor in SUPP)
    // then resample to 90 minutes.
    let out = borsa.history(&inst, req).await.unwrap();

    // Expected buckets: [0..90m) -> last close=3.0 (from 60m), [90m..180m) -> last close=4.0 (90m)
    let ts: Vec<i64> = out.candles.iter().map(|c| c.ts.timestamp()).collect();
    assert_eq!(ts, vec![0, 5400]);

    let closes: Vec<Decimal> = out.candles.iter().map(|c| c.close.amount()).collect();
    assert_eq!(
        closes,
        vec![Decimal::from(3u8), Decimal::from(4u8)],
        "should reflect the 30m source, not 60m (which would yield 20.0, 30.0)"
    );
}
