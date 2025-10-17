#![cfg(feature = "test-adapters")]

use std::sync::Arc;

use async_trait::async_trait;
use borsa_core::connector::StreamProvider;
use borsa_core::{AssetKind, Instrument};
use tokio::sync::{Mutex, oneshot, watch};

use borsa_yfinance::adapter::{CloneArcAdapters, YfStream};

// A spy YfStream implementation that only stops when its own handle.stop() is called.
// It exposes a watch channel so the test can observe whether stop propagated.
struct SpyYfStream {
    inner_stop_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    stopped_tx: watch::Sender<bool>,
}

#[async_trait]
impl YfStream for SpyYfStream {
    async fn start(
        &self,
        _symbols: &[String],
    ) -> Result<
        (
            borsa_core::StreamHandle,
            tokio::sync::mpsc::Receiver<borsa_core::QuoteUpdate>,
        ),
        borsa_core::BorsaError,
    > {
        let (_tx, rx) = tokio::sync::mpsc::channel::<borsa_core::QuoteUpdate>(16);

        // An upstream background task that only ends when it receives inner_stop_rx.
        let (inner_stop_tx, inner_stop_rx) = oneshot::channel::<()>();
        {
            let mut guard = self.inner_stop_tx.lock().await;
            *guard = Some(inner_stop_tx);
        }
        let stopped_tx = self.stopped_tx.clone();
        let join = tokio::spawn(async move {
            // Wait until told to stop; this simulates a WS task blocked on IO unless stopped.
            if inner_stop_rx.await == Ok(()) {
                let _ = stopped_tx.send(true);
            } else {
                // Sender dropped: treat as not properly stopped
            }
        });

        let (stop_tx, stop_rx) = oneshot::channel::<()>();
        let inner_for_wrapper = self.inner_stop_tx.clone();
        let wrapper = tokio::spawn(async move {
            // Only forward when we actually receive a stop signal (Ok(())),
            // not when the sender is dropped. This simulates a real upstream
            // handle that doesn't stop just because a wrapper is dropped.
            if stop_rx.await == Ok(())
                && let Some(tx) = inner_for_wrapper.lock().await.take()
            {
                let _ = tx.send(());
            }
            let _ = join.await;
        });

        Ok((borsa_core::StreamHandle::new(wrapper, stop_tx), rx))
    }
}

// Minimal adapter that returns our spy YfStream; other capabilities default to unsupported.
#[derive(Clone)]
struct SpyAdapter {
    stream: Arc<dyn YfStream>,
}

impl CloneArcAdapters for SpyAdapter {
    fn clone_arc_stream(&self) -> Arc<dyn YfStream> {
        self.stream.clone()
    }
}

#[tokio::test]
async fn stream_quotes_stop_propagates_to_upstream_handle() {
    // Arrange: spy wiring
    let (stopped_tx, mut stopped_rx) = watch::channel(false);
    let inner_stop_tx: Arc<Mutex<Option<oneshot::Sender<()>>>> = Arc::new(Mutex::new(None));
    let spy_stream = Arc::new(SpyYfStream {
        inner_stop_tx: inner_stop_tx.clone(),
        stopped_tx,
    });
    let connector = borsa_yfinance::YfConnector::from_adapter(&SpyAdapter { stream: spy_stream });

    // Act: start and then stop the stream via the connector's returned handle
    let (handle, _rx) = connector
        .stream_quotes(&[
            Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid test instrument")
        ])
        .await
        .expect("stream started");

    handle.stop().await; // request stop on returned handle

    // Assert: upstream should have been stopped (becomes true within a short time)
    let ok = tokio::time::timeout(std::time::Duration::from_millis(200), async {
        loop {
            if *stopped_rx.borrow() {
                break true;
            }
            if stopped_rx.changed().await.is_err() {
                break false;
            }
        }
    })
    .await
    .unwrap_or(false);

    assert!(ok, "expected stop() to propagate to upstream stream handle");

    // Cleanup: if it didn't stop (in current buggy impl), try to stop upstream to avoid leaks
    if !*stopped_rx.borrow()
        && let Some(tx) = inner_stop_tx.lock().await.take()
    {
        let _ = tx.send(());
    }
}
