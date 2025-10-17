use crate::helpers::MockConnector;
use crate::helpers::usd;
use borsa::Borsa;
use borsa_core::{AssetKind, Candle, HistoryRequest, HistoryResponse, Interval, Range};
use chrono::TimeZone;

fn c(ts: i64, close: i64) -> Candle {
    let ts_dt = chrono::Utc.timestamp_opt(ts, 0).unwrap();
    let m = usd(&close.to_string());
    Candle {
        ts: ts_dt,
        open: m.clone(),
        high: m.clone(),
        low: m.clone(),
        close: m,
        volume: None,
    }
}

const fn supported_d1() -> &'static [borsa_core::Interval] {
    &[borsa_core::Interval::D1]
}

#[tokio::test]
async fn weekly_request_falls_back_to_daily_and_resamples() {
    const DAY: i64 = 86_400;
    // Build a connector that only supports D1 and asserts it is asked for D1.
    let c = MockConnector::builder()
        .name("daily_only")
        .with_history_intervals(supported_d1())
        .with_history_fn(|_i, _r| {
            // Return 5 daily candles Mon..Fri in one week
            Ok(HistoryResponse {
                candles: vec![
                    c(0 * DAY, 1),    // Monday
                    c(1 * DAY, 2),    // Tuesday
                    c(2 * DAY, 3),    // Wednesday
                    c(3 * DAY, 4),    // Thursday
                    c(4 * DAY, 5),    // Friday
                ],
                actions: vec![],
                adjusted: false,
                meta: None,
            })
        })
        .build();

    let borsa = Borsa::builder().with_connector(c).build();
    let inst = crate::helpers::instrument("X", AssetKind::Equity);

    // Request weekly bars; provider only supports D1 -> router should fetch D1 and resample to W1.
    let req = HistoryRequest::try_from_range(Range::D7, Interval::W1).unwrap();
    let out = borsa.history(&inst, req).await.unwrap();

    // Expect one weekly candle starting Monday 00:00 UTC.
    assert_eq!(out.candles.len(), 1);
    assert_eq!(out.candles[0].ts.timestamp(), 0);
    // Close should be last close of the week (Friday)
    assert_eq!(out.candles[0].close.amount().to_string(), "5");
}


