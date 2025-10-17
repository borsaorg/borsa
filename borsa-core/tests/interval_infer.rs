use borsa_core::timeseries::infer::{estimate_step_seconds, is_subdaily};
use borsa_core::{Candle, Currency, IsoCurrency, Money};
use chrono::DateTime;
use proptest::prelude::*;
use rust_decimal::Decimal;

fn m0() -> Money {
    Money::new(Decimal::ZERO, Currency::Iso(IsoCurrency::USD)).unwrap()
}

fn c(ts: i64) -> Candle {
    let px = m0();
    Candle {
        ts: DateTime::from_timestamp(ts, 0).unwrap(),
        open: px.clone(),
        high: px.clone(),
        low: px.clone(),
        close: px,
        close_unadj: None,
        volume: None,
    }
}

// Removed mirrored oracle; replaced with metamorphic properties below.

proptest! {
    #[test]
    fn constant_step_with_noise(step_idx in 0usize..4, n in 5usize..100, rev in any::<bool>(), kinds in proptest::collection::vec(0u8..=3, 4..100)) {
        let steps = [60i64, 300, 3600, 86_400];
        let step = steps[step_idx];

        // Build deltas with a small bounded amount of noise (duplicates/short/long)
        let mut noise_budget: usize = ((n.saturating_sub(1)) / 5).min(3);
        let mut ts: Vec<i64> = Vec::with_capacity(n);
        let mut cur: i64 = 0;
        ts.push(cur);
        for &k in kinds.iter().take(n.saturating_sub(1)) {
            let d = if noise_budget == 0 || k == 0 { step } else {
                noise_budget -= 1;
                match k {
                    1 => 0, // duplicate
                    2 => if step > 120 { 60 } else { 1 }, // short gap
                    _ => step.saturating_mul(2), // long gap
                }
            };
            cur = cur.saturating_add(d);
            ts.push(cur);
        }
        if rev { ts.reverse(); }
        let candles: Vec<Candle> = ts.into_iter().map(c).collect();

        // Expected: unique mode if present; otherwise lower median of positive deltas
        let mut ts_sorted: Vec<_> = candles.iter().map(|c| c.ts).collect();
        ts_sorted.sort();
        let mut deltas: Vec<i64> = Vec::new();
        for w in ts_sorted.windows(2) {
            let d = (w[1] - w[0]).num_seconds();
            if d > 0 { deltas.push(d); }
        }
        deltas.sort_unstable();
        let expected = if deltas.is_empty() {
            None
        } else {
            // find mode and count best candidates
            let mut best_delta = deltas[0];
            let mut best_count = 0usize;
            let mut candidates = 0usize;
            let mut cur = deltas[0];
            let mut cur_count = 1usize;
            for &d in deltas.iter().skip(1) {
                if d == cur { cur_count += 1; continue; }
                if cur_count > best_count { best_count = cur_count; best_delta = cur; candidates = 1; }
                else if cur_count == best_count { candidates = candidates.saturating_add(1); }
                cur = d; cur_count = 1;
            }
            if cur_count > best_count { best_delta = cur; candidates = 1; }
            else if cur_count == best_count { candidates = candidates.saturating_add(1); }
            if candidates == 1 { Some(best_delta) } else {
                let mid = deltas.len()/2; if deltas.len()%2==1 { Some(deltas[mid]) } else { Some(deltas[mid-1]) }
            }
        };
        prop_assert_eq!(estimate_step_seconds(candles), expected);
    }

    #[test]
    fn sparse_outlier_immunity_daily(n in 5usize..50, short_gaps in 0usize..=2) {
        let step = 86_400i64;
        let mut ts: Vec<i64> = Vec::with_capacity(n);
        let mut cur: i64 = 0;
        ts.push(cur);
        // Put up to two short gaps among daily steps
        let mut used_shorts = 0usize;
        for i in 0..(n-1) {
            let d = if used_shorts < short_gaps && (i % 7 == 0) { used_shorts += 1; 60 } else { step };
            cur += d;
            ts.push(cur);
        }
        let candles: Vec<Candle> = ts.into_iter().map(c).collect();
        prop_assert_eq!(estimate_step_seconds(candles.clone()), Some(step));
        prop_assert!(!is_subdaily(&candles));
    }

    #[test]
    fn median_tie_breaker(d1 in prop::sample::select(vec![60i64, 120, 300]), d2 in prop::sample::select(vec![120i64, 300, 600]), k in 1usize..50) {
        // Ensure two distinct deltas with equal frequency; expect lower median (the smaller delta)
        let (a, b) = match d1.cmp(&d2) {
            std::cmp::Ordering::Less => (d1, d2),
            std::cmp::Ordering::Greater => (d2, d1),
            std::cmp::Ordering::Equal => (60i64, 120i64),
        };
        let mut ts: Vec<i64> = Vec::with_capacity(2*k + 1);
        let mut cur: i64 = 0;
        ts.push(cur);
        for _ in 0..k { cur += a; ts.push(cur); }
        for _ in 0..k { cur += b; ts.push(cur); }
        // Permute order to ensure order-invariance
        ts.rotate_left(k / 2);
        let candles: Vec<Candle> = ts.into_iter().map(c).collect();
        prop_assert_eq!(estimate_step_seconds(candles), Some(a));
    }
}

proptest! {
    #[test]
    fn degenerate_sequences_empty_and_singleton(len in prop::sample::select(vec![0usize, 1usize])) {
        let mut ts = Vec::with_capacity(len);
        for i in 0..len { ts.push(i64::try_from(i).unwrap()); }
        let candles: Vec<Candle> = ts.into_iter().map(c).collect();
        prop_assert_eq!(estimate_step_seconds(candles.as_slice().to_vec()), None);
        prop_assert!(!is_subdaily(&candles));
    }
}

proptest! {
    #[test]
    fn translation_invariance_for_step_and_subdaily(
        steps in prop::sample::select(vec![60i64, 120, 300, 600, 3600, 86_400]),
        n in 3usize..100,
        offset in -1_000_000i64..1_000_000i64
    ) {
        let mut ts: Vec<i64> = Vec::with_capacity(n);
        let mut cur: i64 = 0;
        ts.push(cur);
        for _ in 1..n { cur = cur.saturating_add(steps); ts.push(cur); }
        let candles: Vec<Candle> = ts.iter().copied().map(c).collect();
        let shifted: Vec<Candle> = ts.iter().copied().map(|t| c(t + offset)).collect();

        prop_assert_eq!(estimate_step_seconds(candles.clone()), estimate_step_seconds(shifted.clone()));
        prop_assert_eq!(is_subdaily(&candles), is_subdaily(&shifted));
    }
}
