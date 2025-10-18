use borsa_core::{
    Candle, Currency, HistoryMeta, HistoryResponse, IsoCurrency, Money, merge_candles_by_priority,
    merge_history,
};
use chrono::{DateTime, Utc};
use proptest::prelude::*;
use rust_decimal::Decimal;
use std::collections::{BTreeMap, BTreeSet};

fn money_usd_cents(cents: i64) -> Money {
    Money::new(Decimal::new(cents, 2), Currency::Iso(IsoCurrency::USD)).unwrap()
}

fn arb_ts() -> impl Strategy<Value = DateTime<Utc>> {
    (-2_000_000_000i64..2_000_000_000i64).prop_map(|s| DateTime::from_timestamp(s, 0).unwrap())
}

fn arb_candle() -> impl Strategy<Value = Candle> {
    (arb_ts(), 0i64..100_000i64).prop_map(|(ts, c)| {
        let px = money_usd_cents(c);
        Candle {
            ts,
            open: px.clone(),
            high: px.clone(),
            low: px.clone(),
            close: px,
            close_unadj: None,
            volume: None,
        }
    })
}

fn arb_response() -> impl Strategy<Value = HistoryResponse> {
    (
        proptest::collection::vec(arb_candle(), 0..200),
        any::<bool>(),
    )
        .prop_map(|(candles, adjusted)| HistoryResponse {
            candles,
            actions: vec![],
            adjusted,
            meta: Some(HistoryMeta {
                timezone: Some(chrono_tz::UTC),
                utc_offset_seconds: Some(0),
            }),
        })
}

proptest! {
    #[test]
    fn first_wins_invariant(responses in proptest::collection::vec(arb_response(), 0..6)) {
        let mut first_by_ts: BTreeMap<i64, Candle> = BTreeMap::new();
        for r in &responses {
            for c in &r.candles {
                first_by_ts.entry(c.ts.timestamp()).or_insert_with(|| c.clone());
            }
        }
        let merged = merge_history(responses).unwrap();
        // Sorted order and first-wins at collisions
        let mut prev = None;
        for c in &merged.candles {
            if let Some(p) = prev { prop_assert!(p <= c.ts.timestamp()); }
            prev = Some(c.ts.timestamp());
            if let Some(exp) = first_by_ts.get(&c.ts.timestamp()) {
                prop_assert_eq!(c.close.amount(), exp.close.amount());
            }
            // close_unadj must be cleared in merged output
            prop_assert!(c.close_unadj.is_none());
        }
    }

    #[test]
    fn adjusted_flag_depends_on_contributing_sources(
        a in arb_response(),
        b in arb_response(),
        c in arb_response()
    ) {
        // Make b fully overlapped by a on timestamps to simulate non-contributing
        let mut b = b;
        if !a.candles.is_empty()
            && let Some(ts0) = a.candles.first().map(|x| x.ts.timestamp()) {
                for (i, bc) in b.candles.iter_mut().enumerate() {
                    let i64_i = i64::try_from(i).unwrap_or(i64::MAX);
                    bc.ts = DateTime::from_timestamp(ts0 + i64_i, 0).unwrap();
                }
            }
        let merged = merge_history([a.clone(), b.clone(), c.clone()]).unwrap();

        // Recompute adjusted across only contributing responses
        let a_ts: BTreeSet<i64> = a.candles.iter().map(|x| x.ts.timestamp()).collect();
        let b_ts: BTreeSet<i64> = b.candles.iter().map(|x| x.ts.timestamp()).collect();
        let c_ts: BTreeSet<i64> = c.candles.iter().map(|x| x.ts.timestamp()).collect();
        let union: BTreeSet<i64> = a_ts.union(&c_ts).copied().collect();
        let ab: BTreeSet<i64> = a_ts.union(&b_ts).copied().collect();
        let contributed_b = ab.len() > a_ts.len();
        let contributed_c = union.len() > a_ts.len();
        let expected_adjusted = a.adjusted
            && (b.adjusted || !contributed_b)
            && (c.adjusted || !contributed_c);
        prop_assert_eq!(merged.adjusted, expected_adjusted);
    }

    #[test]
    fn merge_candles_first_series_wins_on_ts_collisions(
        ts in proptest::collection::vec(arb_ts(), 1..50),
        a_vals in proptest::collection::vec(0i64..10_000i64, 1..50),
        b_vals in proptest::collection::vec(0i64..10_000i64, 1..50),
    ) {
        // Use identical timestamps across both series with different close values
        let mut ts_sorted = ts;
        ts_sorted.sort();
        let s1: Vec<Candle> = ts_sorted.iter().zip(a_vals.iter().cycle()).map(|(t, v)| {
            let px = money_usd_cents(*v);
            Candle { ts: *t, open: px.clone(), high: px.clone(), low: px.clone(), close: px, close_unadj: None, volume: None }
        }).collect();
        let s2: Vec<Candle> = ts_sorted.iter().zip(b_vals.iter().cycle()).map(|(t, v)| {
            let px = money_usd_cents(*v + 1);
            Candle { ts: *t, open: px.clone(), high: px.clone(), low: px.clone(), close: px, close_unadj: None, volume: None }
        }).collect();
        let merged = merge_candles_by_priority([s1.clone(), s2]).unwrap();
        // Expect the first series values
        for (i, c) in merged.iter().enumerate() {
            let exp = &s1[i];
            prop_assert_eq!(c.ts, exp.ts);
            prop_assert_eq!(c.close.amount(), exp.close.amount());
            prop_assert!(c.close_unadj.is_none());
        }
    }

    #[test]
    fn meta_selection_first_non_none_wins(
        a in arb_response(),
        b in arb_response(),
        c in arb_response(),
        which in 0usize..3,
        perm_idx in 0usize..6,
    ) {
        // Ensure only one response has Some(meta)
        let mut a = a; let mut b = b; let mut c = c;
        a.meta = None; b.meta = None; c.meta = None;
        let meta = Some(HistoryMeta { timezone: Some(chrono_tz::Europe::Rome), utc_offset_seconds: None });
        match which { 0 => a.meta = meta, 1 => b.meta = meta, _ => c.meta = meta }

        let all = [a, b, c];
        let perms: [[usize;3];6] = [[0,1,2],[0,2,1],[1,0,2],[1,2,0],[2,0,1],[2,1,0]];
        let order = perms[perm_idx];
        let ordered = [all[order[0]].clone(), all[order[1]].clone(), all[order[2]].clone()];
        let merged = merge_history(ordered).unwrap();

        // Find expected meta as first in order with Some
        let expected = [all[order[0]].meta.clone(), all[order[1]].meta.clone(), all[order[2]].meta.clone()]
            .into_iter().flatten().next();
        prop_assert_eq!(merged.meta, expected);
    }

    #[test]
    fn merged_actions_are_deduplicated_and_sorted(
        ts in proptest::collection::vec(arb_ts(), 1..50),
        cents in proptest::collection::vec(0i64..10_000i64, 1..50)
    ) {
        // Build a set of duplicated actions across multiple responses
        let mut ts_sorted = ts;
        ts_sorted.sort();
        let mut actions: Vec<borsa_core::Action> = Vec::new();
        for (i, t) in ts_sorted.iter().enumerate() {
            let c = cents[i % cents.len()];
            let amt = money_usd_cents(c);
            actions.push(borsa_core::Action::Dividend { ts: *t, amount: amt.clone() });
            actions.push(borsa_core::Action::Dividend { ts: *t, amount: amt });
        }
        // Duplicate the same across multiple responses to test cross-response dedup
        let r1 = HistoryResponse { candles: vec![], actions: actions.clone(), adjusted: true, meta: None };
        let r2 = HistoryResponse { candles: vec![], actions: actions.clone(), adjusted: true, meta: None };
        let r3 = HistoryResponse { candles: vec![], actions, adjusted: true, meta: None };

        let merged = merge_history([r1.clone(), r2.clone(), r3.clone()]).unwrap();
        let expected = borsa_core::dedup_actions([r1.actions, r2.actions, r3.actions].into_iter().flatten().collect());
        prop_assert_eq!(merged.actions, expected);
    }
}

proptest! {
    #[test]
    fn merge_identity_no_op(r in arb_response()) {
        let merged = merge_history([r.clone()]).unwrap();

        // Build expected by applying merge semantics to a single response:
        // - first-wins on duplicate timestamps (preserve first occurrence by original order)
        // - sorted by timestamp
        // - close_unadj cleared
        // - actions deduplicated
        // - adjusted equals input.adjusted if any candle contributed, else false
        let mut seen = std::collections::BTreeSet::new();
        let mut uniq_first: Vec<Candle> = Vec::new();
        for c in &r.candles {
            if seen.insert(c.ts.timestamp()) {
                uniq_first.push(c.clone());
            }
        }
        uniq_first.sort_by_key(|c| c.ts);
        for c in &mut uniq_first { c.close_unadj = None; }

        let expected_actions = borsa_core::dedup_actions(r.actions.clone());
        let expected_adjusted = if uniq_first.is_empty() { false } else { r.adjusted };

        prop_assert_eq!(merged.candles, uniq_first);
        prop_assert_eq!(merged.actions, expected_actions);
        prop_assert_eq!(merged.adjusted, expected_adjusted);
        prop_assert_eq!(merged.meta, r.meta);
    }
}
