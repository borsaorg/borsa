use chrono::TimeDelta;
use paft::market::responses::history::Candle;

/// Estimate a representative step (in seconds) from positive adjacent timestamp
/// deltas in the input series.
///
/// Prefer the mode (most frequent positive delta); if there is no unique mode,
/// return the lower median.
///
/// Examples
///
/// Unique mode (60s):
///
/// ```
/// use borsa_core::{estimate_step_seconds, Candle, Money, Currency, IsoCurrency};
/// use chrono::{DateTime, Utc};
/// use rust_decimal::Decimal;
///
/// fn t(sec: i64) -> DateTime<Utc> { DateTime::from_timestamp(sec, 0).unwrap() }
/// fn money_usd(v: f64) -> Money {
///     Money::new(Decimal::from_f64_retain(v).unwrap(), Currency::Iso(IsoCurrency::USD)).unwrap()
/// }
///
/// let mk = |ts: i64| Candle { ts: t(ts), open: money_usd(1.0), high: money_usd(1.0), low: money_usd(1.0), close: money_usd(1.0), close_unadj: None, volume: None };
/// // Adjacent deltas: 60,60,60,120,180  => unique mode is 60
/// let candles = vec![mk(0), mk(60), mk(120), mk(180), mk(300), mk(480)];
/// assert_eq!(estimate_step_seconds(candles), Some(60));
/// ```
///
/// No unique mode: fall back to lower median (60s):
///
/// ```
/// use borsa_core::{estimate_step_seconds, Candle, Money, Currency, IsoCurrency};
/// use chrono::{DateTime, Utc};
/// use rust_decimal::Decimal;
///
/// fn t(sec: i64) -> DateTime<Utc> { DateTime::from_timestamp(sec, 0).unwrap() }
/// fn money_usd(v: f64) -> Money {
///     Money::new(Decimal::from_f64_retain(v).unwrap(), Currency::Iso(IsoCurrency::USD)).unwrap()
/// }
///
/// let mk = |ts: i64| Candle { ts: t(ts), open: money_usd(1.0), high: money_usd(1.0), low: money_usd(1.0), close: money_usd(1.0), close_unadj: None, volume: None };
/// // Adjacent deltas: 60,60,120,120  => lower median is 60
/// let candles = vec![mk(0), mk(60), mk(120), mk(240), mk(360)];
/// assert_eq!(estimate_step_seconds(candles), Some(60));
/// ```
///
/// The input order does not matter; duplicates are ignored. Returns `None` if
/// fewer than two distinct timestamps are present.
#[must_use]
pub fn estimate_step_seconds(mut candles: Vec<Candle>) -> Option<i64> {
    if candles.len() < 2 {
        return None;
    }
    candles.sort_by_key(|c| c.ts);

    let mut deltas: Vec<i64> = Vec::with_capacity(candles.len().saturating_sub(1));
    let mut last = candles[0].ts;
    for c in candles.into_iter().skip(1) {
        let dt: TimeDelta = c.ts - last;
        if dt > TimeDelta::zero() {
            deltas.push(dt.num_seconds());
            last = c.ts;
        }
    }
    if deltas.is_empty() {
        return None;
    }
    deltas.sort_unstable();

    // Prefer the mode (most frequent positive delta). If there is no unique mode,
    // return the lower median to ensure we pick an actually observed cadence.
    let mut best_delta: i64 = deltas[0];
    let mut best_count: usize = 0;
    let mut num_best_candidates: usize = 0;

    let mut cur_delta: i64 = deltas[0];
    let mut cur_count: usize = 1;
    for &d in deltas.iter().skip(1) {
        if d == cur_delta {
            cur_count += 1;
            continue;
        }
        if cur_count > best_count {
            best_count = cur_count;
            best_delta = cur_delta;
            num_best_candidates = 1;
        } else if cur_count == best_count {
            num_best_candidates = num_best_candidates.saturating_add(1);
        }
        cur_delta = d;
        cur_count = 1;
    }
    // Finalize last run (avoid assigning to best_count to keep lints happy)
    if cur_count > best_count {
        best_delta = cur_delta;
        num_best_candidates = 1;
    } else if cur_count == best_count {
        num_best_candidates = num_best_candidates.saturating_add(1);
    }

    if num_best_candidates == 1 {
        return Some(best_delta);
    }

    // Lower median
    let mid = deltas.len() / 2;
    if deltas.len() % 2 == 1 {
        Some(deltas[mid])
    } else {
        Some(deltas[mid - 1])
    }
}

/// Heuristic: determine if a series is sub-daily.
///
/// Hardened criterion: require evidence of sub-daily cadence.
/// Returns `true` only if BOTH conditions hold:
/// - At least 3 adjacent deltas are strictly less than 86,400 seconds (1 day)
/// - At least 60% of adjacent deltas are strictly less than 86,400 seconds
#[must_use]
pub fn is_subdaily(candles: &[Candle]) -> bool {
    const DAY: i64 = 86_400;
    if candles.len() < 2 {
        return false;
    }

    // Compute positive adjacent deltas after sorting; ignore duplicates.
    let mut ts: Vec<_> = candles.iter().map(|c| c.ts).collect();
    ts.sort();
    let mut deltas: Vec<i64> = Vec::with_capacity(ts.len().saturating_sub(1));
    let mut last = ts[0];
    for &cur in ts.iter().skip(1) {
        let dt: TimeDelta = cur - last;
        if dt > TimeDelta::zero() {
            deltas.push(dt.num_seconds());
            last = cur;
        }
    }

    if deltas.is_empty() {
        return false;
    }

    let total: usize = deltas.len();
    let subdaily: usize = deltas.iter().filter(|&&d| d > 0 && d < DAY).count();

    // Evidence thresholds
    let min_count: usize = 3;
    let min_ratio_num: usize = 3; // 60% = 3/5 as integer comparison against total*5
    let min_ratio_den: usize = 5;

    if subdaily < min_count {
        return false;
    }
    // subdaily/total >= 3/5  =>  subdaily * 5 >= total * 3
    subdaily.saturating_mul(min_ratio_den) >= total.saturating_mul(min_ratio_num)
}
