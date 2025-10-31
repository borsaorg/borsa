use borsa::{Borsa, Resampling};
use borsa_core::{AssetKind, HistoryRequest};

use crate::helpers::{BTC_USD, ETH_USD, m_hist};

#[tokio::test]
async fn resamples_intraday_into_daily() {
    // Single connector with 3 intraday candles on day 0 and 2 on day 1
    // ts values in seconds since epoch
    let c = m_hist("C", &[10, 20, 30, 86_410, 86_420]);

    // With resampling enabled
    let borsa = Borsa::builder()
        .with_connector(c)
        .resampling(Resampling::Daily)
        .build()
        .unwrap();

    let inst = crate::helpers::instrument(&BTC_USD, AssetKind::Crypto);
    let req =
        HistoryRequest::try_from_range(borsa_core::Range::D1, borsa_core::Interval::I1m).unwrap();

    let out = borsa.history(&inst, req).await.unwrap();
    // Expect two daily candles at day starts (0 and 86400)
    assert_eq!(out.candles.len(), 2);
    assert_eq!(out.candles[0].ts.timestamp(), 0);
    assert_eq!(out.candles[1].ts.timestamp(), 86_400);
}

#[tokio::test]
async fn without_resample_returns_original_granularity() {
    let c = m_hist("C", &[10, 20, 30, 86_410, 86_420]);

    let borsa = Borsa::builder().with_connector(c).build().unwrap();

    let inst = crate::helpers::instrument(&ETH_USD, AssetKind::Crypto);
    let req =
        HistoryRequest::try_from_range(borsa_core::Range::D1, borsa_core::Interval::I1m).unwrap();

    let out = borsa.history(&inst, req).await.unwrap();
    // No resampling -> all five original candles (first-wins merge semantics still apply)
    assert_eq!(out.candles.len(), 5);
}
