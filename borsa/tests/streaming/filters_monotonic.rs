use borsa::router::streaming::filters::MonotonicGate;
use borsa_core::{QuoteUpdate, Symbol};

fn mk_u(symbol: &str, ts_secs: i64) -> QuoteUpdate {
    QuoteUpdate {
        symbol: Symbol::new(symbol.to_string()),
        ts: chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
            chrono::NaiveDateTime::from_timestamp_opt(ts_secs, 0).unwrap(),
            chrono::Utc,
        ),
        ..Default::default()
    }
}

#[tokio::test]
async fn monotonic_gate_allows_first_and_equal_and_blocks_older() {
    let gate = MonotonicGate::new();
    let s = "AAPL";

    assert!(gate.allow(&mk_u(s, 1000)).await);
    // equal timestamp allowed
    assert!(gate.allow(&mk_u(s, 1000)).await);
    // older than last rejects
    assert!(!gate.allow(&mk_u(s, 999)).await);
    // newer allowed, and advances
    assert!(gate.allow(&mk_u(s, 1001)).await);
}

#[tokio::test]
async fn monotonic_gate_is_per_symbol() {
    let gate = MonotonicGate::new();

    assert!(gate.allow(&mk_u("AAPL", 1000)).await);
    // Different symbol does not inherit timestamp
    assert!(gate.allow(&mk_u("MSFT", 900)).await);
}


