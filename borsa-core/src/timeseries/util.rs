//! Shared helpers for candle series normalization and invariants.

use crate::BorsaError;
use paft::market::responses::history::Candle;
use paft::money::Currency;

/// Remove per-candle raw close provenance from a slice of candles.
pub fn strip_unadjusted(candles: &mut [Candle]) {
    for c in candles {
        c.close_unadj = None;
    }
}

/// Ensure OHLC currencies within a single candle are identical.
///
/// # Errors
/// Returns `Err(BorsaError::Data)` if the candle's `open`, `high`, `low`, and `close`
/// do not all share the same currency.
pub fn ensure_candle_currency_uniform(c: &Candle) -> Result<(), BorsaError> {
    let cur = c.open.currency();
    if cur != c.high.currency() || cur != c.low.currency() || cur != c.close.currency() {
        return Err(BorsaError::Data("currency mismatch within candle".into()));
    }
    Ok(())
}

/// Ensure all candles in the series have identical currency (and each candle is internally uniform).
/// Returns the common currency on success.
///
/// # Errors
/// - Returns `Err(BorsaError::Data)` if any candle has mixed currencies across its OHLC fields.
/// - Returns `Err(BorsaError::Data)` if multiple candles in the series use different currencies.
/// - Returns `Err(BorsaError::Data)` if the series is empty.
pub fn ensure_series_currency_uniform(candles: &[Candle]) -> Result<Currency, BorsaError> {
    let mut series_cur: Option<Currency> = None;
    for c in candles {
        ensure_candle_currency_uniform(c)?;
        let oc = c.open.currency().clone();
        if let Some(ref cur) = series_cur {
            if cur != &oc {
                return Err(BorsaError::Data("currency mismatch across series".into()));
            }
        } else {
            series_cur = Some(oc);
        }
    }
    series_cur.ok_or_else(|| BorsaError::Data("empty series has no currency".into()))
}
