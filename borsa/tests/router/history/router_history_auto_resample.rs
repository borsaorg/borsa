use borsa::{Borsa, Resampling};
use borsa_core::{AssetKind, HistoryRequest, Interval, Range};

use crate::helpers::m_hist;

#[tokio::test]
async fn auto_resamples_when_series_is_subdaily() {
    // Intraday-like timestamps: three on day 0, one on day 1
    let c = m_hist("C", &[10, 20, 30, 86_410]);

    let borsa = Borsa::builder()
        .with_connector(c)
        .auto_resample_subdaily_to_daily(true)
        .build();

    let inst = crate::helpers::instrument("BTC-USD", AssetKind::Crypto);
    let out = borsa
        .history(
            &inst,
            HistoryRequest::try_from_range(Range::D1, Interval::I1m).unwrap(),
        )
        .await
        .unwrap();

    // Expect two daily candles (day 0 -> ts=0, day 1 -> ts=86_400)
    assert_eq!(out.candles.len(), 2);
    assert_eq!(out.candles[0].ts.timestamp(), 0);
    assert_eq!(out.candles[1].ts.timestamp(), 86_400);
}

#[tokio::test]
async fn auto_does_not_trigger_for_daily_series() {
    let day = 86_400;
    let c = m_hist("C", &[0, day, 2 * day]);

    let borsa = Borsa::builder()
        .with_connector(c)
        .auto_resample_subdaily_to_daily(true)
        .build();

    let inst = crate::helpers::instrument("ETH-USD", AssetKind::Crypto);
    let out = borsa
        .history(
            &inst,
            HistoryRequest::try_from_range(Range::D1, Interval::D1).unwrap(),
        )
        .await
        .unwrap();

    // Already daily -> unchanged (3 candles)
    assert_eq!(out.candles.len(), 3);
}

#[tokio::test]
async fn explicit_daily_or_weekly_overrides_auto() {
    let c = m_hist("C", &[10, 20, 30, 86_410]);

    // Daily explicit overrides auto (still daily, but via explicit path)
    let borsa_daily = Borsa::builder()
        .with_connector(c.clone())
        .resampling(Resampling::Daily)
        .auto_resample_subdaily_to_daily(true)
        .build();

    let inst = crate::helpers::instrument("AAPL", AssetKind::Equity);
    let out_d = borsa_daily
        .history(
            &inst,
            HistoryRequest::try_from_range(Range::D1, Interval::I1m).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(out_d.candles.len(), 2);

    // Weekly explicit overrides auto and daily
    let borsa_weekly = Borsa::builder()
        .with_connector(c)
        .resampling(Resampling::Weekly)
        .resampling(Resampling::Daily)
        .auto_resample_subdaily_to_daily(true)
        .build();

    let out_w = borsa_weekly
        .history(
            &inst,
            HistoryRequest::try_from_range(Range::D1, Interval::I1m).unwrap(),
        )
        .await
        .unwrap();
    assert!(
        out_w.candles.len() <= 2,
        "weekly resample should not expand rows"
    );
}
