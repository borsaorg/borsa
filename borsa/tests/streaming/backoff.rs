use borsa::router::streaming::backoff::jitter_wait;

#[test]
fn jitter_wait_within_bounds() {
    let base_ms = 1000;
    let jitter_percent = 10; // 10%
    for _ in 0..100 {
        let v = jitter_wait(base_ms, jitter_percent);
        assert!(v >= base_ms);
        assert!(v < base_ms + (base_ms * jitter_percent as u64) / 100 + 1);
    }
}

#[test]
fn jitter_wait_zero_percent_is_identity() {
    let base_ms = 500;
    for _ in 0..10 {
        let v = jitter_wait(base_ms, 0);
        assert_eq!(v, base_ms);
    }
}


