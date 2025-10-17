use borsa_core::{Action, Currency, IsoCurrency, Money, dedup_actions};
use chrono::{DateTime, Utc};
use proptest::prelude::*;
use rust_decimal::Decimal;
use std::collections::HashSet;

#[allow(dead_code)]
fn money_usd(v: f64) -> Money {
    Money::new(
        Decimal::from_f64_retain(v).unwrap(),
        Currency::Iso(IsoCurrency::USD),
    )
    .unwrap()
}

fn arb_ts() -> impl Strategy<Value = DateTime<Utc>> {
    // Broaden to include negative timestamps
    (-2_000_000_000i64..2_000_000_000i64).prop_map(|s| DateTime::from_timestamp(s, 0).unwrap())
}

fn arb_money_usd() -> impl Strategy<Value = Money> {
    // Non-negative, limited precision
    (0u64..1_000_000u64).prop_map(|cents| {
        let amt = Decimal::new(i64::try_from(cents).unwrap_or(i64::MAX), 2);
        Money::new(amt, Currency::Iso(IsoCurrency::USD)).unwrap()
    })
}

fn arb_action() -> impl Strategy<Value = Action> {
    prop_oneof![
        (arb_ts(), arb_money_usd()).prop_map(|(ts, amount)| Action::Dividend { ts, amount }),
        (arb_ts(), (1u32..=100u32), (1u32..=100u32)).prop_map(|(ts, numerator, denominator)| {
            Action::Split {
                ts,
                numerator,
                denominator,
            }
        }),
        (arb_ts(), arb_money_usd()).prop_map(|(ts, gain)| Action::CapitalGain { ts, gain }),
    ]
}

proptest! {
    #[test]
    fn dedup_idempotent(actions in proptest::collection::vec(arb_action(), 0..200)) {
        let once = dedup_actions(actions);
        let twice = dedup_actions(once.clone());
        prop_assert_eq!(once, twice);
    }

    #[test]
    fn dedup_matches_unique_canonical_keys_with_stable_order(actions in proptest::collection::vec(arb_action(), 0..200)) {
        // Build canonical key per action
        let key = |a: &Action| -> (i64, u8, String) {
            match a {
                Action::Dividend { ts, amount } => (ts.timestamp(), 0, format!("{}|{:?}", amount.amount(), amount.currency())),
                Action::Split { ts, numerator, denominator } => (ts.timestamp(), 1, format!("{numerator}|{denominator}")),
                Action::CapitalGain { ts, gain } => (ts.timestamp(), 2, format!("{}|{:?}", gain.amount(), gain.currency())),
            }
        };

        // Compute stable unique set of keys in expected order
        let mut keys: Vec<(i64, u8, String)> = actions.iter().map(&key).collect();
        keys.sort();
        keys.dedup();

        let out = dedup_actions(actions.clone());
        let out_keys: Vec<(i64, u8, String)> = out.iter().map(&key).collect();

        prop_assert_eq!(out_keys.as_slice(), keys.as_slice());
        prop_assert_eq!(out.len(), keys.len());

        // Also assert every output action existed in the input by key
        let input_key_set: HashSet<(i64, u8, String)> = actions.iter().map(key).collect();
        for k in &out_keys { prop_assert!(input_key_set.contains(k)); }
    }
}

proptest! {
    #[test]
    fn dedup_associative_commutative(a in proptest::collection::vec(arb_action(), 0..100),
                                     b in proptest::collection::vec(arb_action(), 0..100)) {
        let left = dedup_actions([a.clone(), b.clone()].into_iter().flatten().collect());
        let right = dedup_actions(dedup_actions(a.clone()).into_iter().chain(dedup_actions(b.clone())).collect());
        prop_assert_eq!(left, right);

        let ab = dedup_actions([a.clone(), b.clone()].into_iter().flatten().collect());
        let ba = dedup_actions([b, a].into_iter().flatten().collect());
        prop_assert_eq!(ab, ba);
    }

    #[test]
    fn dedup_is_sorted_by_ts_kind_payload(actions in proptest::collection::vec(arb_action(), 0..200)) {
        let out = dedup_actions(actions);
        // Ensure non-decreasing ordering by (ts, variant key, payload)
        let key = |a: &Action| -> (i64, u8, String) {
            match a {
                Action::Dividend { ts, amount } => (ts.timestamp(), 0, format!("{}|{:?}", amount.amount(), amount.currency())),
                Action::Split { ts, numerator, denominator } => (ts.timestamp(), 1, format!("{numerator}|{denominator}")),
                Action::CapitalGain { ts, gain } => (ts.timestamp(), 2, format!("{}|{:?}", gain.amount(), gain.currency())),
            }
        };
        let mut prev: Option<(i64, u8, String)> = None;
        for a in out {
            let k = key(&a);
            if let Some(p) = prev.as_ref() { prop_assert!(p <= &k); }
            prev = Some(k);
        }
    }
}
