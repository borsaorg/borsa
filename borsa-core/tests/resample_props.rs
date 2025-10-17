use borsa_core::{
    Candle, Currency, HistoryMeta, IsoCurrency, Money, resample_to_daily, resample_to_weekly,
    timeseries::resample::{
        resample_to_daily_with_meta, resample_to_minutes_with_meta, resample_to_weekly_with_meta,
    },
};
use chrono::offset::LocalResult;
use chrono::{DateTime, Datelike, Offset, TimeZone, Timelike, Utc};
use proptest::prelude::*;
use rust_decimal::Decimal;

fn money_usd_cents(cents: i64) -> Money {
    Money::new(Decimal::new(cents, 2), Currency::Iso(IsoCurrency::USD)).unwrap()
}

fn arb_ts() -> impl Strategy<Value = DateTime<Utc>> {
    (-2_000_000_000i64..2_000_000_000i64).prop_map(|s| DateTime::from_timestamp(s, 0).unwrap())
}

fn arb_candle() -> impl Strategy<Value = Candle> {
    // Generate coherent OHLC and optional volume, single currency USD
    (
        arb_ts(),
        0i64..100_000i64,
        0i64..100_000i64,
        0i64..100_000i64,
        0i64..100_000i64,
        prop::option::of(0u64..1_000_000u64),
    )
        .prop_map(|(ts, o, h, l, c, vol)| {
            // Ensure high >= max(open, close), low <= min(open, close)
            let open = money_usd_cents(o);
            let close = money_usd_cents(c);
            let max_oc = if open.amount() > close.amount() {
                open.amount()
            } else {
                close.amount()
            };
            let min_oc = if open.amount() < close.amount() {
                open.amount()
            } else {
                close.amount()
            };
            let high_amt = Decimal::max(max_oc, money_usd_cents(h).amount());
            let low_amt = Decimal::min(min_oc, money_usd_cents(l).amount());
            let high = Money::new(high_amt, Currency::Iso(IsoCurrency::USD)).unwrap();
            let low = Money::new(low_amt, Currency::Iso(IsoCurrency::USD)).unwrap();
            Candle {
                ts,
                open,
                high,
                low,
                close,
                close_unadj: None,
                volume: vol,
            }
        })
}

const fn bucket_day(ts: DateTime<Utc>) -> i64 {
    ts.timestamp().div_euclid(86_400) * 86_400
}

proptest! {
    #[test]
    fn resample_idempotent_all(
        candles in proptest::collection::vec(arb_candle(), 0..300),
        mode in prop::sample::select(vec!["daily", "weekly", "m1", "m5", "m15", "m60"])
    ) {
        let once: Vec<Candle> = match mode {
            "daily" => resample_to_daily(candles),
            "weekly" => resample_to_weekly(candles),
            _ => {
                let mins: i64 = match mode { "m5" => 5, "m15" => 15, "m60" => 60, _ => 1 };
                borsa_core::resample_to_minutes(candles, mins)
            }
        };
        let twice: Vec<Candle> = match mode {
            "daily" => resample_to_daily(once.clone()),
            "weekly" => resample_to_weekly(once.clone()),
            _ => {
                let mins: i64 = match mode { "m5" => 5, "m15" => 15, "m60" => 60, _ => 1 };
                borsa_core::resample_to_minutes(once.clone(), mins)
            }
        };
        prop_assert_eq!(once, twice);
    }

    #[test]
    fn ohlc_rules_daily_or_weekly(
        mut candles in proptest::collection::vec(arb_candle(), 0..300),
        mode in prop::sample::select(vec!["daily", "weekly"])
    ) {
        // Group by selected bucket
        let mut groups: std::collections::BTreeMap<i64, Vec<Candle>> = std::collections::BTreeMap::new();
        let bucket_key = |ts: DateTime<Utc>| -> i64 {
            if mode == "daily" {
                bucket_day(ts)
            } else {
                let day = ts.timestamp().div_euclid(86_400);
                let ws = day - ((day + 3).rem_euclid(7));
                ws * 86_400
            }
        };
        for c in &candles { groups.entry(bucket_key(c.ts)).or_default().push(c.clone()); }

        let out: Vec<Candle> = if mode == "daily" {
            resample_to_daily(std::mem::take(&mut candles))
        } else {
            resample_to_weekly(std::mem::take(&mut candles))
        };
        let mut out_map = std::collections::BTreeMap::new();
        for c in out { out_map.insert(c.ts.timestamp(), c); }

        for (b, group) in groups {
            if group.is_empty() { continue; }
            let first = group.iter().min_by_key(|c| c.ts).unwrap();
            let last = group.iter().max_by_key(|c| c.ts).unwrap();
            let high = group.iter().max_by_key(|c| c.high.amount()).unwrap();
            let low = group.iter().min_by_key(|c| c.low.amount()).unwrap();
            let exp_vol: Option<u64> = if mode == "daily" {
                let vol_sum: Option<u128> = group.iter().filter_map(|c| c.volume.map(u128::from)).reduce(|a,b| a+b);
                vol_sum.and_then(|v| u64::try_from(v.min(u128::from(u64::MAX))).ok())
            } else { None };

            if let Some(rc) = out_map.get(&b) {
                prop_assert_eq!(rc.open.amount(), first.open.amount());
                prop_assert_eq!(rc.close.amount(), last.close.amount());
                prop_assert_eq!(rc.high.amount(), high.high.amount());
                prop_assert_eq!(rc.low.amount(), low.low.amount());
                if mode == "daily" { prop_assert_eq!(rc.volume, exp_vol); }
            }
        }
    }
}

proptest! {
    #[test]
    fn minutes_param_ohlc_rules(
        mut candles in proptest::collection::vec(arb_candle(), 0..300),
        mins in prop::sample::select(vec![1i64, 5, 15, 60])
    ) {
        // Build slow model by grouping seconds since midnight floor by step in UTC
        let step = mins * 60;
        candles.sort_by_key(|c| c.ts);
        let mut groups: std::collections::BTreeMap<i64, Vec<Candle>> = std::collections::BTreeMap::new();
        for c in &candles {
            let b = c.ts.timestamp() - c.ts.timestamp().rem_euclid(step);
            groups.entry(b).or_default().push(c.clone());
        }
        let out = borsa_core::resample_to_minutes(candles, mins);
        let mut out_map = std::collections::BTreeMap::new();
        for c in out { out_map.insert(c.ts.timestamp(), c); }
        for (b, group) in groups {
            if group.is_empty() { continue; }
            let first = group.iter().min_by_key(|c| c.ts).unwrap();
            let last = group.iter().max_by_key(|c| c.ts).unwrap();
            let high = group.iter().max_by_key(|c| c.high.amount()).unwrap();
            let low = group.iter().min_by_key(|c| c.low.amount()).unwrap();
            if let Some(rc) = out_map.get(&b) {
                prop_assert_eq!(rc.open.amount(), first.open.amount());
                prop_assert_eq!(rc.close.amount(), last.close.amount());
                prop_assert_eq!(rc.high.amount(), high.high.amount());
                prop_assert_eq!(rc.low.amount(), low.low.amount());
            }
        }
    }
}

proptest! {
    #[test]
    #[test]
    fn with_meta_dst_alignment(
        mut candles in proptest::collection::vec(arb_candle(), 0..200),
        minutes in prop::sample::select(vec![1i64, 5, 15, 60])
    ) {
        // Choose Europe/Rome for DST tests
        let meta = Some(HistoryMeta { timezone: Some(chrono_tz::Europe::Rome), utc_offset_seconds: None });
        // Focus timestamps within two known DST windows by remapping some to those windows
        // Spring forward gap: last Sunday of March around 02:00 local
        // Fall back overlap: last Sunday of October around 03:00 local
        // We don't need exact dates; we map random candles into +/- 6h windows around typical boundaries
        let rome = chrono_tz::Europe::Rome;
        let map_to_window = |ts: DateTime<Utc>, month: u32, day: u32, hour: u32| {
            let y = 2022; // arbitrary DST year
            let base_utc = rome
                .with_ymd_and_hms(y, month, day, hour, 0, 0)
                .single().map_or_else(|| Utc.with_ymd_and_hms(y, month, day, hour, 0, 0).single().unwrap(), |dt| dt.with_timezone(&Utc));
            // spread within +/- 6 hours
            let delta = (ts.timestamp() % (6*3600)).abs();
            let sign = if ts.timestamp() & 1 == 0 { 1 } else { -1 };
            base_utc + chrono::Duration::seconds(sign * delta)
        };
        for (i, c) in candles.iter_mut().enumerate() {
            if i % 2 == 0 {
                c.ts = map_to_window(c.ts, 3, 27, 1); // around spring forward
            } else {
                c.ts = map_to_window(c.ts, 10, 30, 2); // around fall back
            }
        }

        // For daily/weekly: assert local alignment properties rather than exact UTC ts equality
        // minutes handled separately below to match ambiguity resolution in implementation

        let o_daily = resample_to_daily_with_meta(candles.clone(), meta.as_ref());
        for c in &o_daily {
            let l = c.ts.with_timezone(&rome);
            prop_assert_eq!(l.hour(), 0);
            prop_assert_eq!(l.minute(), 0);
            prop_assert_eq!(l.second(), 0);
        }

        let o_weekly = resample_to_weekly_with_meta(candles.clone(), meta.as_ref());
        for c in &o_weekly {
            let l = c.ts.with_timezone(&rome);
            prop_assert_eq!(l.weekday().num_days_from_monday(), 0);
            prop_assert_eq!(l.hour(), 0);
            prop_assert_eq!(l.minute(), 0);
            prop_assert_eq!(l.second(), 0);
        }

        // Build expected groups using the same ambiguous-resolution rule as implementation
        let step = minutes * 60;
        let mut groups: std::collections::BTreeMap<i64, Vec<Candle>> = std::collections::BTreeMap::new();
        for c in candles.clone() {
            let local = c.ts.with_timezone(&rome);
            let date = local.date_naive();
            let secs = i64::from(local.num_seconds_from_midnight());
            let bucket_sec = secs - secs.rem_euclid(step);
            let midnight = date.and_hms_opt(0,0,0).unwrap();
            let local_bucket = midnight + chrono::Duration::seconds(bucket_sec);
            let mapped = match rome.from_local_datetime(&local_bucket) {
                LocalResult::Single(dt) => dt.with_timezone(&Utc),
                LocalResult::Ambiguous(dt1, dt2) => {
                    let local_offset = local.offset().fix().local_minus_utc();
                    if dt1.offset().fix().local_minus_utc() == local_offset { dt1.with_timezone(&Utc) } else { dt2.with_timezone(&Utc) }
                }
                LocalResult::None => {
                    let bucket = c.ts.timestamp() - c.ts.timestamp().rem_euclid(step);
                    DateTime::from_timestamp(bucket, 0).unwrap()
                }
            };
            groups.entry(mapped.timestamp()).or_default().push(c);
        }

        let out = resample_to_minutes_with_meta(candles, minutes, meta.as_ref());
        let mut out_map = std::collections::BTreeMap::new();
        for c in out { out_map.insert(c.ts.timestamp(), c); }

        for (b, group) in groups {
            if group.is_empty() { continue; }
            let first = group.iter().min_by_key(|c| c.ts).unwrap();
            let last = group.iter().max_by_key(|c| c.ts).unwrap();
            let high = group.iter().max_by_key(|c| c.high.amount()).unwrap();
            let low = group.iter().min_by_key(|c| c.low.amount()).unwrap();
            if let Some(rc) = out_map.get(&b) {
                prop_assert_eq!(rc.open.amount(), first.open.amount());
                prop_assert_eq!(rc.close.amount(), last.close.amount());
                prop_assert_eq!(rc.high.amount(), high.high.amount());
                prop_assert_eq!(rc.low.amount(), low.low.amount());
            } else {
                prop_assert!(false, "missing expected bucket {b}");
            }
        }
    }
}

proptest! {
    #[test]
    fn minutes_guardrails_invalid_step_returns_input(
        mut candles in proptest::collection::vec(arb_candle(), 0..100),
        step in prop::sample::select(vec![0i64, -1, -5, -60])
    ) {
        let input = candles.clone();
        let out = borsa_core::resample_to_minutes(std::mem::take(&mut candles), step);
        prop_assert_eq!(out, input);
    }
}

proptest! {
    #[test]
    fn utc_equivalence_between_with_meta_and_plain(
        candles in proptest::collection::vec(arb_candle(), 0..200),
        mins in prop::sample::select(vec![1i64, 5, 15, 60])
    ) {
        let meta_utc = Some(HistoryMeta { timezone: Some(chrono_tz::UTC), utc_offset_seconds: Some(0) });

        // daily
        let plain_d = resample_to_daily(candles.clone());
        let meta_d = resample_to_daily_with_meta(candles.clone(), meta_utc.as_ref());
        prop_assert_eq!(plain_d, meta_d);

        // weekly
        let plain_w = resample_to_weekly(candles.clone());
        let meta_w = resample_to_weekly_with_meta(candles.clone(), meta_utc.as_ref());
        prop_assert_eq!(plain_w, meta_w);

        // minutes
        let plain_m = borsa_core::resample_to_minutes(candles.clone(), mins);
        let meta_m = resample_to_minutes_with_meta(candles, mins, meta_utc.as_ref());
        prop_assert_eq!(plain_m, meta_m);
    }
}
