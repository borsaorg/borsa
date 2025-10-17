use borsa_core as _borsa_core; // for explicit minutes call
use borsa_core::timeseries::merge::merge_candles_by_priority;
use borsa_core::{
    Candle, Currency, HistoryMeta, HistoryResponse, IsoCurrency, Money, merge_history,
    resample_to_daily, resample_to_weekly,
};
use chrono::{DateTime, Utc};
use proptest::prelude::*;
use rust_decimal::Decimal;
use std::collections::BTreeMap;

fn money(usd: bool, cents: i64) -> Money {
    Money::new(
        Decimal::new(cents, 2),
        if usd {
            Currency::Iso(IsoCurrency::USD)
        } else {
            Currency::Iso(IsoCurrency::EUR)
        },
    )
    .unwrap()
}

fn candle(ts: DateTime<Utc>, usd: bool, cents: i64) -> Candle {
    let px = money(usd, cents);
    Candle {
        ts,
        open: px.clone(),
        high: px.clone(),
        low: px.clone(),
        close: px,
        close_unadj: None,
        volume: None,
    }
}

fn arb_ts() -> impl Strategy<Value = DateTime<Utc>> {
    (-2_000_000_000i64..2_000_000_000i64).prop_map(|s| DateTime::from_timestamp(s, 0).unwrap())
}

proptest! {
    #[test]
    fn merge_currency_invariant(series1 in proptest::collection::vec((arb_ts(), any::<i64>()), 0..50),
                                 series2 in proptest::collection::vec((arb_ts(), any::<i64>()), 0..50),
                                 usd1 in any::<bool>(), usd2 in any::<bool>()) {
        // Build two series with potentially differing currencies
        let s1: Vec<Candle> = series1.iter().map(|(ts, c)| candle(*ts, usd1, c.abs() % 10_000)).collect();
        let s2: Vec<Candle> = series2.iter().map(|(ts, c)| candle(*ts, usd2, (c.abs() + 1) % 10_000)).collect();

        let same_currency = usd1 == usd2 || s1.is_empty() || s2.is_empty();

        // merge_candles_by_priority behavior
        let mc = std::panic::catch_unwind(|| merge_candles_by_priority([s1.clone(), s2.clone()]));
        if same_currency {
            prop_assert!(mc.is_ok());
        } else {
            prop_assert!(mc.is_err());
        }

        // merge_history behavior (wrap the series into HistoryResponse)
        let r1 = HistoryResponse { candles: s1, actions: vec![], adjusted: false, meta: Some(HistoryMeta { timezone: None, utc_offset_seconds: None }) };
        let r2 = HistoryResponse { candles: s2, actions: vec![], adjusted: false, meta: None };
        let mh = std::panic::catch_unwind(|| merge_history([r1, r2]));
        if same_currency {
            prop_assert!(mh.is_ok());
        } else {
            prop_assert!(mh.is_err());
        }
    }


    #[test]
    fn resample_currency_invariant_all(
        candles_raw in proptest::collection::vec((arb_ts(), any::<bool>(), any::<i64>()), 0..200),
        mode in prop::sample::select(vec!["daily", "weekly", "m1", "m5", "m15", "m60"]) ) {
        // Normalize and sort input
        let mut candles: Vec<Candle> = candles_raw.iter().map(|(ts, usd, c)| candle(*ts, *usd, c.abs() % 10_000)).collect();
        candles.sort_by_key(|c| c.ts);

        // Bucket function per mode
        let bucket_key = |ts: DateTime<Utc>| -> i64 {
            match mode {
                "daily" => ts.timestamp().div_euclid(86_400) * 86_400,
                "weekly" => {
                    let day = ts.timestamp().div_euclid(86_400);
                    let ws = day - ((day + 3).rem_euclid(7));
                    ws * 86_400
                }
                _ => {
                    let mins: i64 = match mode { "m5" => 5, "m15" => 15, "m60" => 60, _ => 1 };
                    let step = mins * 60;
                    ts.timestamp() - ts.timestamp().rem_euclid(step)
                }
            }
        };

        // Detect per-bucket and cross-bucket currency changes
        let mut by_bucket: BTreeMap<i64, Vec<&Candle>> = BTreeMap::new();
        for c in &candles { by_bucket.entry(bucket_key(c.ts)).or_default().push(c); }
        let mut spans_currency = false;
        let mut series_currency: Option<Currency> = None;
        let mut cross_bucket_currency_change = false;
        for group in by_bucket.values() {
            let mut cur: Option<Currency> = None;
            for c in group {
                let next = c.open.currency().clone();
                if let Some(prev) = &cur {
                    if prev != &next { spans_currency = true; break; }
                } else { cur = Some(next); }
            }
            if spans_currency { break; }
            if let Some(bucket_cur) = cur {
                if let Some(sc) = &series_currency {
                    if sc != &bucket_cur { cross_bucket_currency_change = true; break; }
                } else {
                    series_currency = Some(bucket_cur);
                }
            }
        }

        // Run selected resampler
        let res = std::panic::catch_unwind(|| match mode {
            "daily" => resample_to_daily(candles.clone()),
            "weekly" => resample_to_weekly(candles.clone()),
            _ => {
                let mins: i64 = match mode { "m5" => 5, "m15" => 15, "m60" => 60, _ => 1 };
                _borsa_core::resample_to_minutes(candles.clone(), mins)
            }
        });

        if spans_currency || cross_bucket_currency_change {
            prop_assert!(res.is_err());
        } else {
            prop_assert!(res.is_ok());
            let out = res.unwrap();
            // Output series-wide currency consistency
            let mut out_cur: Option<Currency> = None;
            for c in &out {
                if let Some(prev) = &out_cur { prop_assert_eq!(prev, c.open.currency()); } else { out_cur = Some(c.open.currency().clone()); }
            }
        }
    }
}
