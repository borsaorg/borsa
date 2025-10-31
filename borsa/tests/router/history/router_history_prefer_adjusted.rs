use borsa::Borsa;

use borsa_core::{AssetKind, Candle, Currency, HistoryRequest, HistoryResponse, Money};
use chrono::TimeZone;
use std::collections::HashMap;

use crate::helpers::{AAPL, MockConnector};

fn candles(ts: &[i64], base: f64) -> Vec<Candle> {
    ts.iter()
        .map(|&t| Candle {
            ts: chrono::Utc.timestamp_opt(t, 0).unwrap(),
            open: Money::from_canonical_str(
                &(base + f64::from(i32::try_from(t).unwrap())).to_string(),
                Currency::Iso(borsa_core::IsoCurrency::USD),
            )
            .unwrap(),
            high: Money::from_canonical_str(
                &(base + f64::from(i32::try_from(t).unwrap())).to_string(),
                Currency::Iso(borsa_core::IsoCurrency::USD),
            )
            .unwrap(),
            low: Money::from_canonical_str(
                &(base + f64::from(i32::try_from(t).unwrap())).to_string(),
                Currency::Iso(borsa_core::IsoCurrency::USD),
            )
            .unwrap(),
            close: Money::from_canonical_str(
                &(base + f64::from(i32::try_from(t).unwrap())).to_string(),
                Currency::Iso(borsa_core::IsoCurrency::USD),
            )
            .unwrap(),
            close_unadj: None,
            volume: None,
        })
        .collect()
}

#[tokio::test]
async fn prefers_adjusted_series_over_non_adjusted_on_overlap() {
    // Unadjusted provider first in insertion/priority order
    let unadj = MockConnector::builder()
        .name("raw")
        .returns_history_ok(HistoryResponse {
            candles: candles(&[1, 2, 3], 0.0), // closes: 1,2,3
            actions: vec![],
            adjusted: false,
            meta: None,
        })
        .build();

    // Adjusted provider second, but should be preferred when flag is enabled
    let adj = MockConnector::builder()
        .name("adj")
        .returns_history_ok(HistoryResponse {
            candles: candles(&[1, 2, 3], 10.0), // closes: 11,12,13
            actions: vec![],
            adjusted: true,
            meta: None,
        })
        .build();

    let borsa = Borsa::builder()
        .with_connector(unadj)
        .with_connector(adj)
        .prefer_adjusted_history(true)
        .build()
        .unwrap();

    let inst = crate::helpers::instrument(&AAPL, AssetKind::Equity);
    let out = borsa
        .history(
            &inst,
            HistoryRequest::try_from_range(borsa_core::Range::D1, borsa_core::Interval::D1)
                .unwrap(),
        )
        .await
        .unwrap();

    // On overlap, the adjusted series should win (e.g., close at ts=2 is 12.0, not 2.0)
    let by_ts: HashMap<_, _> = out.candles.iter().map(|c| (c.ts.timestamp(), c)).collect();
    assert_eq!(by_ts[&2].close.amount().to_string(), "12");
}

#[tokio::test]
async fn default_behavior_keeps_priority_if_flag_not_set() {
    // Same data, but without enabling prefer_adjusted_history
    let unadj = MockConnector::builder()
        .name("raw")
        .returns_history_ok(HistoryResponse {
            candles: candles(&[1, 2, 3], 0.0),
            actions: vec![],
            adjusted: false,
            meta: None,
        })
        .build();
    let adj = MockConnector::builder()
        .name("adj")
        .returns_history_ok(HistoryResponse {
            candles: candles(&[1, 2, 3], 10.0),
            actions: vec![],
            adjusted: true,
            meta: None,
        })
        .build();

    let borsa = Borsa::builder()
        .with_connector(unadj)
        .with_connector(adj)
        // .prefer_adjusted_history(false) // default
        .build()
        .unwrap();

    let inst = crate::helpers::instrument(&AAPL, AssetKind::Equity);
    let out = borsa
        .history(
            &inst,
            HistoryRequest::try_from_range(borsa_core::Range::D1, borsa_core::Interval::D1)
                .unwrap(),
        )
        .await
        .unwrap();

    // With default (flag off), first connector (raw) wins on overlap (ts=2 -> 2.0)
    let by_ts: HashMap<_, _> = out.candles.iter().map(|c| (c.ts.timestamp(), c)).collect();
    assert_eq!(by_ts[&2].close.amount().to_string(), "2");
}

#[tokio::test]
async fn prefer_adjusted_drops_unadjusted_even_when_non_overlapping() {
    // Unadjusted provider first provides older, non-overlapping data
    let unadj = MockConnector::builder()
        .name("raw")
        .returns_history_ok(HistoryResponse {
            candles: candles(&[1, 2], 0.0),
            actions: vec![],
            adjusted: false,
            meta: None,
        })
        .build();

    // Adjusted provider second provides newer, non-overlapping data
    let adj = MockConnector::builder()
        .name("adj")
        .returns_history_ok(HistoryResponse {
            candles: candles(&[3, 4], 10.0),
            actions: vec![],
            adjusted: true,
            meta: None,
        })
        .build();

    let borsa = Borsa::builder()
        .with_connector(unadj)
        .with_connector(adj)
        .prefer_adjusted_history(true)
        .build()
        .unwrap();

    let inst = crate::helpers::instrument(&AAPL, AssetKind::Equity);
    let out = borsa
        .history(
            &inst,
            HistoryRequest::try_from_range(borsa_core::Range::D1, borsa_core::Interval::D1)
                .unwrap(),
        )
        .await
        .unwrap();

    let ts: Vec<i64> = out.candles.iter().map(|c| c.ts.timestamp()).collect();
    assert_eq!(ts, vec![3, 4], "only adjusted series should remain");
}

#[tokio::test]
async fn no_preference_keeps_first_series_adjustedness_even_when_non_overlapping() {
    // Unadjusted provider first provides older, non-overlapping data
    let unadj = MockConnector::builder()
        .name("raw")
        .returns_history_ok(HistoryResponse {
            candles: candles(&[1, 2], 0.0),
            actions: vec![],
            adjusted: false,
            meta: None,
        })
        .build();

    // Adjusted provider second provides newer, non-overlapping data
    let adj = MockConnector::builder()
        .name("adj")
        .returns_history_ok(HistoryResponse {
            candles: candles(&[3, 4], 10.0),
            actions: vec![],
            adjusted: true,
            meta: None,
        })
        .build();

    let borsa = Borsa::builder()
        .with_connector(unadj)
        .with_connector(adj)
        .build()
        .unwrap();

    let inst = crate::helpers::instrument(&AAPL, AssetKind::Equity);
    let out = borsa
        .history(
            &inst,
            HistoryRequest::try_from_range(borsa_core::Range::D1, borsa_core::Interval::D1)
                .unwrap(),
        )
        .await
        .unwrap();

    let ts: Vec<i64> = out.candles.iter().map(|c| c.ts.timestamp()).collect();
    assert_eq!(
        ts,
        vec![1, 2],
        "only first-series adjustedness (unadjusted) should remain"
    );
}
