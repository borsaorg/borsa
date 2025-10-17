use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use borsa::{BackoffConfig, Borsa};
use borsa_core::{AssetKind, BorsaConnector, BorsaError, Instrument};

struct CountingFailStreamConnector {
    attempts: Arc<AtomicUsize>,
    events: Arc<Mutex<Vec<tokio::time::Instant>>>,
}

#[async_trait]
impl borsa_core::connector::StreamProvider for CountingFailStreamConnector {
    async fn stream_quotes(
        &self,
        _instruments: &[Instrument],
    ) -> Result<
        (
            borsa_core::StreamHandle,
            tokio::sync::mpsc::Receiver<borsa_core::QuoteUpdate>,
        ),
        BorsaError,
    > {
        let prev = self.attempts.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut events) = self.events.lock() {
            events.push(tokio::time::Instant::now());
        }
        if prev == 0 {
            let (tx, rx) = tokio::sync::mpsc::channel::<borsa_core::QuoteUpdate>(1);
            drop(tx);
            let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
            let join = tokio::spawn(async move {
                let _ = stop_rx.await;
            });
            Ok((borsa_core::StreamHandle::new(join, stop_tx), rx))
        } else {
            Err(BorsaError::Other("start failed".into()))
        }
    }
}

#[async_trait]
impl BorsaConnector for CountingFailStreamConnector {
    fn name(&self) -> &'static str {
        "counting_fail"
    }
    fn supports_kind(&self, _kind: AssetKind) -> bool {
        true
    }
    fn as_stream_provider(&self) -> Option<&dyn borsa_core::connector::StreamProvider> {
        Some(self)
    }
}

#[tokio::test(start_paused = true)]
async fn stream_backoff_exponential_no_jitter() {
    use tokio::time::{Duration, advance};
    async fn yield_until(attempts: &AtomicUsize, n: usize) {
        for _ in 0..20 {
            if attempts.load(Ordering::SeqCst) >= n {
                break;
            }
            tokio::task::yield_now().await;
        }
    }

    let attempts = Arc::new(AtomicUsize::new(0));
    let events = Arc::new(Mutex::new(Vec::new()));
    let conn = Arc::new(CountingFailStreamConnector {
        attempts: attempts.clone(),
        events: events.clone(),
    });

    let borsa = Borsa::builder()
        .with_connector(conn)
        .backoff(BackoffConfig {
            min_backoff_ms: 10,
            max_backoff_ms: 1_000,
            factor: 2,
            jitter_percent: 0,
        })
        .build();

    let inst = crate::helpers::instrument("X", AssetKind::Equity);
    let (handle, _rx) = borsa
        .stream_quotes_with_backoff(&[inst], None)
        .await
        .expect("stream setup ok");

    let failures = |attempts: &AtomicUsize| attempts.load(Ordering::SeqCst).saturating_sub(1);

    for _ in 0..10 {
        if attempts.load(Ordering::SeqCst) >= 1 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(failures(&attempts), 0);

    advance(Duration::from_millis(9)).await;
    yield_until(&attempts, 2).await;
    assert_eq!(failures(&attempts), 0);

    advance(Duration::from_millis(1)).await;
    yield_until(&attempts, 2).await;
    assert_eq!(failures(&attempts), 1);

    advance(Duration::from_millis(9)).await;
    yield_until(&attempts, 3).await;
    assert_eq!(failures(&attempts), 1);

    advance(Duration::from_millis(1)).await;
    yield_until(&attempts, 3).await;
    assert_eq!(failures(&attempts), 2);

    advance(Duration::from_millis(19)).await;
    yield_until(&attempts, 4).await;
    assert_eq!(failures(&attempts), 2);

    advance(Duration::from_millis(1)).await;
    yield_until(&attempts, 4).await;
    assert!(failures(&attempts) >= 3);

    handle.stop().await;
}

#[tokio::test(start_paused = true)]
async fn stream_backoff_jitter_bounds() {
    use tokio::time::{Duration, advance};
    async fn yield_until(attempts: &AtomicUsize, n: usize) {
        for _ in 0..20 {
            if attempts.load(Ordering::SeqCst) >= n {
                break;
            }
            tokio::task::yield_now().await;
        }
    }

    let attempts = Arc::new(AtomicUsize::new(0));
    let events = Arc::new(Mutex::new(Vec::new()));
    let conn = Arc::new(CountingFailStreamConnector {
        attempts: attempts.clone(),
        events: events.clone(),
    });

    let backoff = BackoffConfig {
        min_backoff_ms: 100,
        max_backoff_ms: 10_000,
        factor: 2,
        jitter_percent: 50,
    };
    let borsa = Borsa::builder()
        .with_connector(conn)
        .backoff(backoff)
        .build();

    let inst = crate::helpers::instrument("X", AssetKind::Equity);
    let (handle, _rx) = borsa
        .stream_quotes_with_backoff(&[inst], None)
        .await
        .expect("stream setup ok");

    let failures = |attempts: &AtomicUsize| attempts.load(Ordering::SeqCst).saturating_sub(1);

    for _ in 0..10 {
        if attempts.load(Ordering::SeqCst) >= 1 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(failures(&attempts), 0);

    advance(Duration::from_millis(99)).await;
    yield_until(&attempts, 2).await;
    assert_eq!(failures(&attempts), 0);

    advance(Duration::from_millis(51)).await; // surpass upper bound (100 + 49) deterministically
    yield_until(&attempts, 2).await;
    assert!(failures(&attempts) >= 1);

    let mut before = failures(&attempts);
    advance(Duration::from_millis(99)).await; // strictly below next lower bound (100)
    yield_until(&attempts, before + 2).await;
    assert_eq!(failures(&attempts), before);

    advance(Duration::from_millis(51)).await; // surpass upper bound (100 + 49)
    yield_until(&attempts, before + 2).await;
    assert!(failures(&attempts) > before);

    before = failures(&attempts);
    advance(Duration::from_millis(199)).await; // strictly below next lower bound (200)
    yield_until(&attempts, before + 2).await;
    assert_eq!(failures(&attempts), before);

    advance(Duration::from_millis(101)).await; // reach and surpass next upper bound (200 + 100)
    yield_until(&attempts, before + 2).await;
    assert!(failures(&attempts) > before);

    handle.stop().await;
}
