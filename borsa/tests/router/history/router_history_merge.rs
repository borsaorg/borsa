use borsa::Borsa;
use borsa_core::{AssetKind, HistoryRequest, Interval, Range};
use rust_decimal::Decimal;
use std::collections::HashMap;

use crate::helpers::{BTC_USD, m_hist};

#[tokio::test]
async fn merges_adjacent_ranges() {
    let a = m_hist("A", &[1, 2, 3]);
    let b = m_hist("B", &[4, 5]);

    let borsa = Borsa::builder()
        .with_connector(a)
        .with_connector(b)
        .build()
        .unwrap();

    let inst = crate::helpers::instrument(&BTC_USD, AssetKind::Crypto);
    let req = HistoryRequest::try_from_range(Range::D5, Interval::D1).unwrap();

    let merged = borsa.history(&inst, req).await.unwrap();
    assert_eq!(merged.candles.len(), 5);
    assert_eq!(merged.candles[0].ts.timestamp(), 1);
    assert_eq!(merged.candles[4].ts.timestamp(), 5);
}

#[tokio::test]
async fn prefers_first_on_overlap() {
    let a = m_hist("A", &[1, 2, 3]);
    let b = m_hist("B", &[2, 3, 4]);

    let borsa = Borsa::builder()
        .with_connector(a)
        .with_connector(b)
        .build()
        .unwrap();

    let inst = crate::helpers::instrument(&BTC_USD, AssetKind::Crypto);
    let req = HistoryRequest::try_from_range(Range::D5, Interval::D1).unwrap();

    let merged = borsa.history(&inst, req).await.unwrap();
    assert_eq!(merged.candles.len(), 4);
    let by_ts: HashMap<_, _> = merged
        .candles
        .iter()
        .map(|c| (c.ts.timestamp(), c))
        .collect();
    assert_eq!(by_ts[&2].close.amount(), Decimal::from(2u8));
}
