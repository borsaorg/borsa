use crate::helpers::MockConnector;
use crate::helpers::usd;
use borsa::Borsa;
use borsa_core::{AssetKind, Candle, HistoryRequest, HistoryResponse, Interval, Range};
use chrono::TimeZone;

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

const fn supported_20m() -> &'static [borsa_core::Interval] {
    &[borsa_core::Interval::I20m]
}

#[tokio::test]
async fn router_rejects_non_divisible_intraday_request() {
    let c = MockConnector::builder()
        .name("twenty_minute")
        .with_history_intervals(supported_20m())
        .with_history_fn(|_i, _r| {
            Ok(HistoryResponse {
                candles: vec![c(0), c(1_200), c(2_400)],
                actions: vec![],
                adjusted: false,
                meta: None,
            })
        })
        .build();

    let borsa = Borsa::builder().with_connector(c).build().unwrap();
    let inst = crate::helpers::instrument("X", AssetKind::Equity);

    let req = HistoryRequest::try_from_range(Range::D1, Interval::I45m).unwrap();
    let err = borsa.history(&inst, req).await.unwrap_err();

    match err {
        borsa_core::BorsaError::AllProvidersFailed(errors) => {
            assert_eq!(errors.len(), 1);
            match &errors[0] {
                borsa_core::BorsaError::Connector { connector, error } => {
                    assert_eq!(connector, "twenty_minute");
                    assert!(matches!(
                        error.as_ref(),
                        borsa_core::BorsaError::Unsupported { capability }
                            if capability == "history interval (intraday too fine for provider)"
                    ));
                }
                other => panic!("expected connector error, got {other:?}"),
            }
        }
        other => panic!("expected AllProvidersFailed error, got {other:?}"),
    }
}
