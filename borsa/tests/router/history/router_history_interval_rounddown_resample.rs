use crate::helpers::MockConnector;
use crate::helpers::usd;
use borsa::Borsa;
use borsa_core::{AssetKind, Candle, HistoryRequest, HistoryResponse, Interval, Range};
use chrono::TimeZone;
use rust_decimal::Decimal;

fn c(ts: i64) -> Candle {
    let ts_dt = chrono::Utc.timestamp_opt(ts, 0).unwrap();
    let m = usd(&ts.to_string());
    Candle {
        ts: ts_dt,
        open: m.clone(),
        high: m.clone(),
        low: m.clone(),
        close: m,
        close_unadj: None,
        volume: None,
    }
}

// Connector only "supports" 1-minute, Borsa should request 1m and then resample to 2m.
const fn supported_1m() -> &'static [borsa_core::Interval] {
    &[borsa_core::Interval::I1m]
}

#[tokio::test]
async fn router_rounds_down_and_resamples_to_requested_minutes() {
    let c = MockConnector::builder()
        .name("one_min")
        .with_history_intervals(supported_1m())
        .with_history_fn(|_i, _r| {
            Ok(HistoryResponse {
                candles: vec![c(60), c(180), c(240)],
                actions: vec![],
                adjusted: false,
                meta: None,
            })
        })
        .build();
    let borsa = Borsa::builder().with_connector(c).build();
    let inst = crate::helpers::instrument("X", AssetKind::Equity);

    let req = HistoryRequest::try_from_range(Range::D1, Interval::I2m).unwrap(); // request 2-minute bars
    // (other flags at defaults)

    let out = borsa.history(&inst, req).await.unwrap();

    // Buckets at 0, 120, 240 with left-closed, right-open binning.
    let ts: Vec<_> = out.candles.iter().map(|c| c.ts.timestamp()).collect();
    assert_eq!(ts, vec![0, 120, 240]);

    let closes: Vec<Decimal> = out.candles.iter().map(|c| c.close.amount()).collect();
    // <- Important: 120s candle is part of the NEXT bucket, so the first bucket closes at 60.
    assert_eq!(
        closes,
        vec![
            Decimal::from(60u8),
            Decimal::from(180u16),
            Decimal::from(240u16)
        ]
    );
}
