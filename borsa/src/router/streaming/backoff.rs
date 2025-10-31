use rand::Rng;

/// Temporary shim to ease migration from legacy helper
pub fn jitter_wait(base_ms: u64, jitter_percent: u32) -> u64 {
    let jitter_range = if jitter_percent == 0 {
        1
    } else {
        std::cmp::max(1, (base_ms.saturating_mul(u64::from(jitter_percent))) / 100)
    };
    let mut rng = rand::rng();
    base_ms + rng.random_range(0..jitter_range)
}
