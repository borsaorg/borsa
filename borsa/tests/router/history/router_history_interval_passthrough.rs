use crate::helpers::usd;
use crate::helpers::{MockConnector, X};
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

const fn supported_15m() -> &'static [borsa_core::Interval] {
    &[borsa_core::Interval::I15m]
}

#[tokio::test]
async fn router_passes_through_supported_interval() {
    let c = MockConnector::builder()
        .name("pass")
        .with_history_intervals(supported_15m())
        .with_history_fn(|_i, _r| {
            Ok(HistoryResponse {
                candles: vec![c(0), c(900), c(1800)], // 15m boundaries
                actions: vec![],
                adjusted: false,
                meta: None,
            })
        })
        .build();
    let borsa = Borsa::builder().with_connector(c).build().unwrap();
    let inst = crate::helpers::instrument(&X, AssetKind::Equity);

    let req = HistoryRequest::try_from_range(Range::D1, Interval::I15m).unwrap();
    // (other flags at defaults)

    let out = borsa.history(&inst, req).await.unwrap();
    let ts: Vec<_> = out.candles.iter().map(|c| c.ts.timestamp()).collect();
    assert_eq!(ts, vec![0, 900, 1800]);

    let closes: Vec<Decimal> = out.candles.iter().map(|c| c.close.amount()).collect();
    assert_eq!(
        closes,
        vec![
            Decimal::from(0u8),
            Decimal::from(900u16),
            Decimal::from(1800u16)
        ]
    );
}
