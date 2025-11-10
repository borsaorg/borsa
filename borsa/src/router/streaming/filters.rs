use std::collections::HashMap;
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use borsa_core::Symbol;

type GateEntry = (chrono::DateTime<chrono::Utc>, Instant);
type GateMap = HashMap<String, GateEntry>;
type GateState = Arc<Mutex<GateMap>>;

pub struct MonotonicGate {
    state: GateState,
}

const REAPER_INTERVAL: Duration = Duration::from_secs(60 * 15);
const ENTRY_TTL: Duration = Duration::from_secs(60 * 60 * 24);

impl MonotonicGate {
    pub fn new() -> Self {
        let state: GateState = Arc::new(Mutex::new(HashMap::new()));

        let weak: Weak<Mutex<GateMap>> = Arc::downgrade(&state);

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(REAPER_INTERVAL).await;
                if let Some(state_arc) = weak.upgrade() {
                    let mut guard = state_arc.lock().await;
                    let now = Instant::now();
                    guard.retain(|_, (_, last_seen)| now.duration_since(*last_seen) <= ENTRY_TTL);
                } else {
                    break;
                }
            }
        });

        Self { state }
    }

    pub async fn allow(&self, key: String, ts: chrono::DateTime<chrono::Utc>) -> bool {
        use std::collections::hash_map::Entry;
        let mut guard = self.state.lock().await;
        let now = Instant::now();
        match guard.entry(key) {
            Entry::Occupied(mut e) => {
                let (prev_ts, last_seen) = e.get_mut();
                if ts < *prev_ts {
                    *last_seen = now;
                    return false;
                }
                if ts > *prev_ts {
                    *prev_ts = ts;
                }
                *last_seen = now;
                true
            }
            Entry::Vacant(e) => {
                e.insert((ts, now));
                true
            }
        }
    }

    pub async fn reset_symbols<'a, I>(&self, symbols: I)
    where
        I: IntoIterator<Item = &'a Symbol>,
    {
        let mut guard = self.state.lock().await;
        for sym in symbols {
            guard.remove(sym.as_str());
        }
    }
}
