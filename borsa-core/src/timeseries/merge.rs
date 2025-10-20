use core::fmt::{self, Write as _};
use std::collections::{BTreeMap, HashSet, btree_map::Entry};
use std::hash::{Hash, Hasher};

use crate::BorsaError;
use chrono::{DateTime, Utc};
use paft::market::action::Action;
use paft::market::responses::history::{Candle, HistoryMeta, HistoryResponse};

/// Merge multiple history responses in priority order (first is highest).
///
/// - Candles are keyed by `ts`; the first appearance wins for duplicates.
/// - Candles are returned sorted by timestamp.
/// - `adjusted` is true only if all sources are adjusted (all-or-nothing).
/// - `meta`: first non-None wins; otherwise None.
/// - Actions are concatenated and de-duplicated by full action identity
///   (same kind, timestamp, and payload), keeping the first identical one.
///
/// # Errors
/// Returns `Err(BorsaError::Data)` if mixed currencies are detected either
/// within a single candle or across the merged output series. Currency
/// consistency is a required invariant for all merged candles.
pub fn merge_history<I>(responses: I) -> Result<HistoryResponse, BorsaError>
where
    I: IntoIterator<Item = HistoryResponse>,
{
    // Track whether all contributing inputs are adjusted; empty input yields `false`.
    let mut adjusted_all: Option<bool> = None;
    // Remember the adjusted flag of the first response that actually contributes candles.
    let mut first_contrib_adjusted: Option<bool> = None;
    let mut meta: Option<HistoryMeta> = None;
    let mut fallback_meta: Option<HistoryMeta> = None;

    let mut candle_map: BTreeMap<DateTime<Utc>, Candle> = BTreeMap::new();
    let mut actions: Vec<Action> = vec![];
    let mut series_currency: Option<paft::money::Currency> = None;

    for mut r in responses.into_iter() {
        let mut response_meta = r.meta.take();
        if fallback_meta.is_none() {
            if let Some(m) = &response_meta {
                fallback_meta = Some(m.clone());
            }
        }
        let mut contributed = false;
        for c in r.candles {
            match candle_map.entry(c.ts) {
                Entry::Vacant(v) => {
                    // Enforce per-candle internal currency consistency and series-wide currency invariants
                    let open_cur_ref = c.open.currency();
                    if !(open_cur_ref == c.high.currency()
                        && open_cur_ref == c.low.currency()
                        && open_cur_ref == c.close.currency())
                    {
                        return Err(BorsaError::Data(
                            "Connector provided mixed-currency history".into(),
                        ));
                    }

                    if let Some(cur) = &series_currency {
                        if cur != open_cur_ref {
                            return Err(BorsaError::Data(
                                "Connector provided mixed-currency history".into(),
                            ));
                        }
                    } else {
                        series_currency = Some(open_cur_ref.clone());
                    }

                    v.insert(c);
                    contributed = true;
                }
                Entry::Occupied(_) => {}
            }
        }
        if contributed {
            adjusted_all = Some(adjusted_all.unwrap_or(true) & r.adjusted);
            if first_contrib_adjusted.is_none() {
                first_contrib_adjusted = Some(r.adjusted);
            }
            if meta.is_none() {
                meta = response_meta.take();
            }
        }
        actions.extend(r.actions.into_iter());
    }

    let empty_series = candle_map.is_empty();
    if meta.is_none() && empty_series {
        meta = fallback_meta;
    }

    let mut candles: Vec<Candle> = candle_map.into_values().collect();
    // Enforce invariant: merged series do not carry per-candle raw close provenance
    for c in &mut candles {
        c.close_unadj = None;
    }

    let actions = dedup_actions(actions);

    let adjusted = match (first_contrib_adjusted, adjusted_all) {
        (Some(first), Some(all)) => first && all,
        _ => false,
    };

    Ok(HistoryResponse {
        candles,
        actions,
        adjusted,
        meta,
    })
}

/// Merge only candles from multiple series (first series has higher priority).
///
/// Returns a vector of candles with first-wins semantics on duplicate timestamps.
/// # Errors
/// Returns `Err(BorsaError::Data)` if mixed currencies are detected either
/// within a single candle or across the merged output series. Currency
/// consistency is required.
pub fn merge_candles_by_priority<I>(series: I) -> Result<Vec<Candle>, BorsaError>
where
    I: IntoIterator<Item = Vec<Candle>>,
{
    let mut map: BTreeMap<DateTime<Utc>, Candle> = BTreeMap::new();
    let mut series_currency: Option<paft::money::Currency> = None;
    for s in series {
        for mut c in s {
            // Enforce per-candle internal currency consistency and series-wide currency invariants
            let open_cur_ref = c.open.currency();
            if !(open_cur_ref == c.high.currency()
                && open_cur_ref == c.low.currency()
                && open_cur_ref == c.close.currency())
            {
                return Err(BorsaError::Data(format!(
                    "Mixed currencies within candle at {}: open={:?} high={:?} low={:?} close={:?}",
                    c.ts,
                    c.open.currency(),
                    c.high.currency(),
                    c.low.currency(),
                    c.close.currency()
                )));
            }

            if let Some(cur) = &series_currency {
                if cur != open_cur_ref {
                    return Err(BorsaError::Data(format!(
                        "Mixed currencies across merged series at {}: expected {:?}, got {:?}",
                        c.ts, cur, open_cur_ref
                    )));
                }
            } else if map.is_empty() {
                series_currency = Some(open_cur_ref.clone());
            }

            // Clear raw close provenance to align with merge_history semantics
            c.close_unadj = None;

            map.entry(c.ts).or_insert(c);
        }
    }
    Ok(map.into_values().collect())
}

/// Helper to write bytes into a hasher using `fmt::Write` without allocating.
struct HashWriter<'a, H: Hasher>(&'a mut H);

impl<H: Hasher> fmt::Write for HashWriter<'_, H> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0.write(s.as_bytes());
        Ok(())
    }
}

fn compare_actions(a: &Action, b: &Action) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    let (ats, ak): (DateTime<Utc>, u8) = match *a {
        Action::Dividend { ts, .. } => (ts, 0),
        Action::Split { ts, .. } => (ts, 1),
        Action::CapitalGain { ts, .. } => (ts, 2),
    };
    let (bts, bk): (DateTime<Utc>, u8) = match *b {
        Action::Dividend { ts, .. } => (ts, 0),
        Action::Split { ts, .. } => (ts, 1),
        Action::CapitalGain { ts, .. } => (ts, 2),
    };

    match ats.cmp(&bts) {
        Ordering::Equal => {}
        non_eq => return non_eq,
    }
    match ak.cmp(&bk) {
        Ordering::Equal => {}
        non_eq => return non_eq,
    }

    match (a, b) {
        (Action::Dividend { amount: a_amt, .. }, Action::Dividend { amount: b_amt, .. }) => {
            let ord = a_amt.amount().cmp(&b_amt.amount());
            if ord == Ordering::Equal {
                format!("{:?}", a_amt.currency()).cmp(&format!("{:?}", b_amt.currency()))
            } else {
                ord
            }
        }
        (Action::CapitalGain { gain: a_gain, .. }, Action::CapitalGain { gain: b_gain, .. }) => {
            let ord = a_gain.amount().cmp(&b_gain.amount());
            if ord == Ordering::Equal {
                format!("{:?}", a_gain.currency()).cmp(&format!("{:?}", b_gain.currency()))
            } else {
                ord
            }
        }
        (
            Action::Split {
                numerator: an,
                denominator: ad,
                ..
            },
            Action::Split {
                numerator: bn,
                denominator: bd,
                ..
            },
        ) => an.cmp(bn).then(ad.cmp(bd)),
        _ => Ordering::Equal,
    }
}

fn hash_action(hasher: &mut std::collections::hash_map::DefaultHasher, a: &Action) {
    match a {
        Action::Dividend { ts, amount } => {
            0u8.hash(hasher);
            ts.timestamp().hash(hasher);
            amount.amount().hash(hasher);
            let mut hw = HashWriter(hasher);
            let _ = write!(&mut hw, "{:?}", amount.currency());
        }
        Action::Split {
            ts,
            numerator,
            denominator,
        } => {
            1u8.hash(hasher);
            ts.timestamp().hash(hasher);
            numerator.hash(hasher);
            denominator.hash(hasher);
        }
        Action::CapitalGain { ts, gain } => {
            2u8.hash(hasher);
            ts.timestamp().hash(hasher);
            gain.amount().hash(hasher);
            let mut hw = HashWriter(hasher);
            let _ = write!(&mut hw, "{:?}", gain.currency());
        }
    }
}

/// Deduplicate actions using full identity (timestamp, variant, payload),
/// keeping a single copy of each distinct action.
///
/// Identity is defined as:
/// - `Dividend`: same `ts` and `amount`.
/// - `Split`: same `ts`, `numerator`, and `denominator`.
/// - `CapitalGain`: same `ts` and `gain`.
#[must_use]
pub fn dedup_actions(mut actions: Vec<Action>) -> Vec<Action> {
    actions.sort_unstable_by(compare_actions);

    let mut seen: HashSet<u64> = HashSet::new();
    let mut out: Vec<Action> = Vec::with_capacity(actions.len());

    for a in actions {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        hash_action(&mut hasher, &a);
        let h = hasher.finish();
        if seen.insert(h) {
            out.push(a);
        }
    }
    out
}

// Inline tests removed; covered by integration/property tests in `borsa-core/tests/`.
