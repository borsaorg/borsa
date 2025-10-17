use borsa::{Borsa, Resampling};
use borsa_core::{AssetKind, HistoryRequest};

use crate::helpers::m_hist;

#[tokio::test]
async fn resamples_into_weekly_monday_start() {
    const DAY: i64 = 86_400;
    // Candles on Tue (day 5), Wed (day 6), and next week's Mon (day 11)
    let c = m_hist("C", &[5 * DAY + 10, 6 * DAY + 20, 11 * DAY + 30]);

    let borsa = Borsa::builder()
        .with_connector(c)
        .resampling(Resampling::Weekly)
        .build()
        .unwrap();

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
    assert_eq!(out.candles[0].ts.timestamp(), 4 * DAY); // week starting Mon day 4
    assert_eq!(out.candles[1].ts.timestamp(), 11 * DAY); // next week Mon
}

#[tokio::test]
async fn weekly_takes_precedence_over_daily() {
    const DAY: i64 = 86_400;
    let c = m_hist("C", &[5 * DAY + 10, 6 * DAY + 20, 11 * DAY + 30]);

    let borsa = Borsa::builder()
        .with_connector(c)
        .resampling(Resampling::Daily)
        .resampling(Resampling::Weekly) // both set -> weekly wins
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

    // Weekly expected two candles, daily would produce three; check weekly took precedence.
    assert_eq!(out.candles.len(), 2);
}
