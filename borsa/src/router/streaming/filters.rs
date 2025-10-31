use std::collections::HashMap;
use tokio::sync::Mutex;

use borsa_core::QuoteUpdate;

pub struct MonotonicGate {
    last_ts: Mutex<HashMap<String, chrono::DateTime<chrono::Utc>>>,
}

impl MonotonicGate {
    pub fn new() -> Self {
        Self {
            last_ts: Mutex::new(HashMap::new()),
        }
    }

    pub async fn allow(&self, update: &QuoteUpdate) -> bool {
        use std::collections::hash_map::Entry;
        let mut guard = self.last_ts.lock().await;
        match guard.entry(update.symbol.as_str().to_string()) {
            Entry::Occupied(mut e) => {
                let prev = *e.get();
                if update.ts < prev {
                    return false;
                }
                if update.ts > prev {
                    *e.get_mut() = update.ts;
                }
                true
            }
            Entry::Vacant(e) => {
                e.insert(update.ts);
                true
            }
        }
    }
}
