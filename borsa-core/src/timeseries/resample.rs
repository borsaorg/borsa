use chrono::Datelike;
use chrono::offset::Offset;
use chrono::{DateTime, NaiveDate, TimeZone, Timelike, Utc};
use paft::market::responses::history::{Candle, HistoryMeta};
use std::convert::TryFrom;
// For resolving local times around DST transitions
use chrono::offset::LocalResult;

const DAY: i64 = 86_400;

const fn week_start_day(day: i64) -> i64 {
    day - ((day + 3).rem_euclid(7))
}

const fn week_start_ts(ts: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let day = ts.timestamp().div_euclid(DAY);
    let ws = week_start_day(day);
    DateTime::from_timestamp(ws * DAY, 0)
}

/// Generic resampler that groups sorted candles by a bucket function and
/// aggregates OHLCV within each bucket.
use crate::BorsaError;

fn resample_by<F>(mut candles: Vec<Candle>, bucket_of: F) -> Result<Vec<Candle>, BorsaError>
where
    F: Fn(DateTime<Utc>) -> Option<DateTime<Utc>>,
{
    if candles.is_empty() {
        return Ok(candles);
    }

    candles.sort_by_key(|c| c.ts);

    let mut out: Vec<Candle> = Vec::new();
    let mut series_currency: Option<paft::money::Currency> = None;

    let mut iter = candles.into_iter();
    let Some(first) = iter.find(|c| bucket_of(c.ts).is_some()) else {
        return Ok(Vec::new());
    };
    let mut cur_bucket = bucket_of(first.ts).unwrap();
    let mut open = first.open;
    let mut high = first.high;
    let mut low = first.low;
    let mut close = first.close;
    let mut vol_sum = first.volume.map(u128::from);

    for c in iter {
        let Some(bucket) = bucket_of(c.ts) else {
            continue;
        };
        if bucket == cur_bucket {
            if !(c.open.currency() == high.currency()
                && c.open.currency() == low.currency()
                && c.open.currency() == close.currency())
            {
                return Err(BorsaError::Data(format!(
                    "Mixed currencies in resample bucket at {}: open={:?} high={:?} low={:?} close={:?}",
                    cur_bucket,
                    c.open.currency(),
                    high.currency(),
                    low.currency(),
                    close.currency()
                )));
            }
            if c.high.amount() > high.amount() {
                high = c.high;
            }
            if c.low.amount() < low.amount() {
                low = c.low;
            }
            close = c.close;
            if let Some(v) = c.volume {
                vol_sum = Some(vol_sum.unwrap_or(0) + u128::from(v));
            }
        } else {
            finalize_bucket(
                &mut out,
                &mut series_currency,
                cur_bucket,
                BucketAgg {
                    open,
                    high,
                    low,
                    close,
                    vol_sum,
                },
            )?;
            cur_bucket = bucket;
            open = c.open;
            high = c.high;
            low = c.low;
            close = c.close;
            vol_sum = c.volume.map(u128::from);
        }
    }

    finalize_bucket(
        &mut out,
        &mut series_currency,
        cur_bucket,
        BucketAgg {
            open,
            high,
            low,
            close,
            vol_sum,
        },
    )?;

    Ok(out)
}

struct BucketAgg {
    open: paft::money::Money,
    high: paft::money::Money,
    low: paft::money::Money,
    close: paft::money::Money,
    vol_sum: Option<u128>,
}

fn finalize_bucket(
    out: &mut Vec<Candle>,
    series_currency: &mut Option<paft::money::Currency>,
    cur_bucket: DateTime<Utc>,
    agg: BucketAgg,
) -> Result<(), BorsaError> {
    if !(agg.open.currency() == agg.high.currency()
        && agg.open.currency() == agg.low.currency()
        && agg.open.currency() == agg.close.currency())
    {
        return Err(BorsaError::Data(format!(
            "Mixed currencies in resample bucket (finalize) at {}: open={:?} high={:?} low={:?} close={:?}",
            cur_bucket,
            agg.open.currency(),
            agg.high.currency(),
            agg.low.currency(),
            agg.close.currency()
        )));
    }
    if let Some(cur) = series_currency {
        if cur != agg.open.currency() {
            return Err(BorsaError::Data(format!(
                "Mixed currencies across resampled series at {}: expected {:?}, got {:?}",
                cur_bucket,
                cur,
                agg.open.currency()
            )));
        }
    } else if out.is_empty() {
        *series_currency = Some(agg.open.currency().clone());
    }
    out.push(Candle {
        ts: cur_bucket,
        open: agg.open,
        high: agg.high,
        low: agg.low,
        close: agg.close,
        close_unadj: None,
        volume: agg
            .vol_sum
            .and_then(|v| u64::try_from(v.min(u128::from(u64::MAX))).ok()),
    });
    Ok(())
}

const fn bucket_day_with_offset(ts: DateTime<Utc>, offset_seconds: i64) -> Option<DateTime<Utc>> {
    let shifted = ts.timestamp() + offset_seconds;
    let day = shifted.div_euclid(DAY);
    let local_day_start = day * DAY - offset_seconds;
    DateTime::from_timestamp(local_day_start, 0)
}

const fn bucket_week_monday_with_offset(
    ts: DateTime<Utc>,
    offset_seconds: i64,
) -> Option<DateTime<Utc>> {
    let shifted_day = (ts.timestamp() + offset_seconds).div_euclid(DAY);
    let ws = week_start_day(shifted_day);
    let local_week_start = ws * DAY - offset_seconds;
    DateTime::from_timestamp(local_week_start, 0)
}

const fn bucket_minutes_with_offset(
    ts: DateTime<Utc>,
    minutes: i64,
    offset_seconds: i64,
) -> Option<DateTime<Utc>> {
    let step = minutes * 60;
    let shifted = ts.timestamp() + offset_seconds;
    let bucket = shifted - shifted.rem_euclid(step);
    let back = bucket - offset_seconds;
    DateTime::from_timestamp(back, 0)
}

fn local_midnight_utc_for_date(
    ts: DateTime<Utc>,
    date: NaiveDate,
    tz: chrono_tz::Tz,
) -> Option<DateTime<Utc>> {
    let naive_midnight = date.and_hms_opt(0, 0, 0)?;
    match tz.from_local_datetime(&naive_midnight) {
        LocalResult::Single(dt) => Some(dt.with_timezone(&Utc)),
        LocalResult::Ambiguous(dt1, _) => Some(dt1.with_timezone(&Utc)),
        LocalResult::None => {
            // Fallback: use UTC day start as a conservative default
            let day = ts.timestamp().div_euclid(DAY);
            DateTime::from_timestamp(day * DAY, 0)
        }
    }
}

fn bucket_day_with_tz(ts: DateTime<Utc>, tz: chrono_tz::Tz) -> Option<DateTime<Utc>> {
    let local = ts.with_timezone(&tz);
    let date = local.date_naive();
    local_midnight_utc_for_date(ts, date, tz)
}

fn bucket_week_monday_with_tz(ts: DateTime<Utc>, tz: chrono_tz::Tz) -> Option<DateTime<Utc>> {
    let local = ts.with_timezone(&tz);
    let date = local.date_naive();
    let days_from_monday = i64::from(local.weekday().num_days_from_monday());
    let week_start_date = date
        .checked_sub_signed(chrono::Duration::days(days_from_monday))
        .unwrap_or(date);
    local_midnight_utc_for_date(ts, week_start_date, tz)
}

fn bucket_minutes_with_tz(
    ts: DateTime<Utc>,
    minutes: i64,
    tz: chrono_tz::Tz,
) -> Option<DateTime<Utc>> {
    let step = minutes * 60;
    let local = ts.with_timezone(&tz);
    let date = local.date_naive();
    let seconds_since_midnight = i64::from(local.num_seconds_from_midnight());
    let bucket_sec = seconds_since_midnight - seconds_since_midnight.rem_euclid(step);
    // Build local midnight and add bucket offset in local time, then convert to UTC
    let naive_midnight = date.and_hms_opt(0, 0, 0)?;
    let local_bucket_naive = naive_midnight + chrono::Duration::seconds(bucket_sec);
    match tz.from_local_datetime(&local_bucket_naive) {
        LocalResult::Single(dt) => Some(dt.with_timezone(&Utc)),
        LocalResult::Ambiguous(dt1, dt2) => {
            // Choose the ambiguous mapping that matches the offset of the original ts in this tz.
            // This preserves distinct buckets across the fall-back overlap hour.
            let local_offset = local.offset().fix().local_minus_utc();
            let dt1_offset = dt1.offset().fix().local_minus_utc();
            if dt1_offset == local_offset {
                Some(dt1.with_timezone(&Utc))
            } else {
                Some(dt2.with_timezone(&Utc))
            }
        }
        LocalResult::None => {
            // Fallback: drop to nearest UTC bucket as conservative behavior
            let bucket = ts.timestamp() - ts.timestamp().rem_euclid(step);
            DateTime::from_timestamp(bucket, 0)
        }
    }
}

fn choose_bucket_day(ts: DateTime<Utc>, meta: Option<&HistoryMeta>) -> Option<DateTime<Utc>> {
    if let Some(m) = meta {
        if let Some(tz) = m.timezone {
            return bucket_day_with_tz(ts, tz);
        }
        if let Some(off) = m.utc_offset_seconds {
            return bucket_day_with_offset(ts, off);
        }
    }
    let day = ts.timestamp().div_euclid(DAY);
    DateTime::from_timestamp(day * DAY, 0)
}

fn choose_bucket_week(ts: DateTime<Utc>, meta: Option<&HistoryMeta>) -> Option<DateTime<Utc>> {
    if let Some(m) = meta {
        if let Some(tz) = m.timezone {
            return bucket_week_monday_with_tz(ts, tz);
        }
        if let Some(off) = m.utc_offset_seconds {
            return bucket_week_monday_with_offset(ts, off);
        }
    }
    week_start_ts(ts)
}

fn choose_bucket_minutes(
    ts: DateTime<Utc>,
    minutes: i64,
    meta: Option<&HistoryMeta>,
) -> Option<DateTime<Utc>> {
    if let Some(m) = meta {
        if let Some(tz) = m.timezone {
            return bucket_minutes_with_tz(ts, minutes, tz);
        }
        if let Some(off) = m.utc_offset_seconds {
            return bucket_minutes_with_offset(ts, minutes, off);
        }
    }
    let step = minutes * 60;
    let bucket = ts.timestamp() - (ts.timestamp().rem_euclid(step));
    DateTime::from_timestamp(bucket, 0)
}

/// Resample arbitrary-interval candles into UTC daily OHLCV.
///
/// - Groups by UTC day (floor(ts / `86_400`)).
/// - Open = first open of the day (earliest ts)
/// - High = max high
/// - Low  = min low
/// - Close = last close of the day (latest ts)
/// - Volume = sum of volumes (ignores `None`; if all `None`, result is `None`)
/// - Output candles have `ts` at the **day start** (00:00:00 UTC).
///
/// # Errors
/// Returns `Err(BorsaError::Data)` if mixed currencies are detected within a
/// bucket or across the resampled output series. All OHLC in every bucket must
/// share the same currency, and all buckets in the output series must share a
/// single currency.
///
/// ```
/// use borsa_core::{resample_to_daily, Candle, Money, Currency, IsoCurrency};
/// use chrono::{DateTime, Utc};
/// use rust_decimal::Decimal;
/// fn t(sec: i64) -> DateTime<Utc> { DateTime::from_timestamp(sec, 0).unwrap() }
/// fn m(v: f64, usd: bool) -> Money { Money::new(Decimal::from_f64_retain(v).unwrap(), if usd { Currency::Iso(IsoCurrency::USD) } else { Currency::Iso(IsoCurrency::EUR) }).unwrap() }
/// let c = |ts: i64, usd: bool| Candle { ts: t(ts), open: m(1.0, usd), high: m(1.0, usd), low: m(1.0, usd), close: m(1.0, usd), close_unadj: None, volume: None };
/// // Two different days with different currencies → panic on finalize of second bucket
/// let res = resample_to_daily(vec![c(0, true), c(86_400, false)]);
/// assert!(res.is_err());
/// ```
pub fn resample_to_daily(candles: Vec<Candle>) -> Result<Vec<Candle>, BorsaError> {
    resample_by(candles, |ts| {
        let day = ts.timestamp().div_euclid(DAY);
        DateTime::from_timestamp(day * DAY, 0)
    })
}

/// Resample to daily buckets using `HistoryMeta` timezone/offset when provided.
///
/// # Errors
/// Returns `Err(BorsaError::Data)` if mixed currencies are detected within a
/// bucket or across the resampled output series.
pub fn resample_to_daily_with_meta(
    candles: Vec<Candle>,
    meta: Option<&HistoryMeta>,
) -> Result<Vec<Candle>, BorsaError> {
    resample_by(candles, move |ts| choose_bucket_day(ts, meta))
}

/// Resample arbitrary-interval candles into UTC weekly OHLCV.
///
/// Weeks start Monday 00:00 UTC.
///
/// - Group key is the Monday-start timestamp: floor(ts / `86_400`) -> day,
///   `week_start_day` = day - ((day + 3) % 7), since 1970-01-01 is Thursday.
///   The output candle `ts` is `week_start_day * 86_400`.
/// - Open  = first open of the week (earliest ts)
/// - High  = max high
/// - Low   = min low
/// - Close = last close of the week (latest ts)
/// - Volume = sum of volumes (ignores `None`; if all `None`, result is `None`)
/// # Errors
/// Returns `Err(BorsaError::Data)` if mixed currencies are detected within a
/// bucket or across the resampled output series.
///
/// ```
/// use borsa_core::{resample_to_weekly, Candle, Money, Currency, IsoCurrency};
/// use chrono::{DateTime, Utc};
/// use rust_decimal::Decimal;
/// fn t(sec: i64) -> DateTime<Utc> { DateTime::from_timestamp(sec, 0).unwrap() }
/// fn m(v: f64, usd: bool) -> Money { Money::new(Decimal::from_f64_retain(v).unwrap(), if usd { Currency::Iso(IsoCurrency::USD) } else { Currency::Iso(IsoCurrency::EUR) }).unwrap() }
/// let c = |ts: i64, usd: bool| Candle { ts: t(ts), open: m(1.0, usd), high: m(1.0, usd), low: m(1.0, usd), close: m(1.0, usd), close_unadj: None, volume: None };
/// // Different weeks and currencies → panic
/// let res = resample_to_weekly(vec![c(0, true), c(7*86_400, false)]);
/// assert!(res.is_err());
/// ```
pub fn resample_to_weekly(candles: Vec<Candle>) -> Result<Vec<Candle>, BorsaError> {
    resample_by(candles, week_start_ts)
}

/// Resample to weekly buckets (Monday start) in market local time using `HistoryMeta` when provided.
///
/// # Errors
/// Returns `Err(BorsaError::Data)` if mixed currencies are detected within a
/// bucket or across the resampled output series.
pub fn resample_to_weekly_with_meta(
    candles: Vec<Candle>,
    meta: Option<&HistoryMeta>,
) -> Result<Vec<Candle>, BorsaError> {
    resample_by(candles, move |ts| choose_bucket_week(ts, meta))
}

/// Resample subdaily candles to an arbitrary minute bucket (e.g., 2m, 90m).
///
/// Assumes input is subdaily. Candles are grouped by `floor(ts / (minutes*60))`.
///
/// # Errors
/// Returns `Err(BorsaError::Data)` if mixed currencies are detected within a
/// bucket or across the resampled output series.
///
/// ```
/// use borsa_core::{resample_to_minutes, Candle, Money, Currency, IsoCurrency};
/// use chrono::{DateTime, Utc};
/// use rust_decimal::Decimal;
/// fn t(sec: i64) -> DateTime<Utc> { DateTime::from_timestamp(sec, 0).unwrap() }
/// fn m(v: f64, usd: bool) -> Money { Money::new(Decimal::from_f64_retain(v).unwrap(), if usd { Currency::Iso(IsoCurrency::USD) } else { Currency::Iso(IsoCurrency::EUR) }).unwrap() }
/// let c = |ts: i64, usd: bool| Candle { ts: t(ts), open: m(1.0, usd), high: m(1.0, usd), low: m(1.0, usd), close: m(1.0, usd), close_unadj: None, volume: None };
/// // Two minute buckets, different currencies → panic
/// let res = resample_to_minutes(vec![c(0, true), c(120, false)], 1);
/// assert!(res.is_err());
/// ```
pub fn resample_to_minutes(candles: Vec<Candle>, minutes: i64) -> Result<Vec<Candle>, BorsaError> {
    if candles.is_empty() || minutes <= 0 {
        return Ok(candles);
    }
    let step = minutes * 60;
    resample_by(candles, move |ts| {
        let bucket = ts.timestamp() - (ts.timestamp().rem_euclid(step));
        DateTime::from_timestamp(bucket, 0)
    })
}

/// Resample to minute buckets using market local time when `HistoryMeta` is provided.
///
/// # Errors
/// Returns `Err(BorsaError::Data)` if mixed currencies are detected within a
/// bucket or across the resampled output series.
pub fn resample_to_minutes_with_meta(
    candles: Vec<Candle>,
    minutes: i64,
    meta: Option<&HistoryMeta>,
) -> Result<Vec<Candle>, BorsaError> {
    if candles.is_empty() || minutes <= 0 {
        return Ok(candles);
    }
    resample_by(candles, move |ts| choose_bucket_minutes(ts, minutes, meta))
}
