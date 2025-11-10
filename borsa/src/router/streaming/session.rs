use std::collections::HashSet;
use std::sync::Arc;

use borsa_core::Symbol;
use borsa_core::stream::StreamHandle;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;

use super::StreamableUpdate;
use super::filters::MonotonicGate;

pub struct SpawnedSession {
    pub join: JoinHandle<()>,
    pub stop_tx: Option<oneshot::Sender<()>>,
}

pub struct SessionManager;

impl SessionManager {
    #[allow(clippy::too_many_arguments)]
    pub fn spawn<T: StreamableUpdate>(
        session_index: usize,
        handle: StreamHandle,
        mut prx: mpsc::Receiver<T>,
        allowed: Option<HashSet<Symbol>>,
        mut stop_watch: watch::Receiver<bool>,
        enforce_monotonic: bool,
        monotonic_gate: Option<Arc<MonotonicGate>>,
        tx_out: mpsc::Sender<T>,
        event_tx: tokio::sync::mpsc::UnboundedSender<(usize, Arc<[Symbol]>)>,
        session_symbols: Arc<[Symbol]>,
    ) -> SpawnedSession {
        let (session_stop_tx, mut session_stop_rx) = oneshot::channel::<()>();

        let monotonic_gate = if enforce_monotonic {
            Some(monotonic_gate.unwrap_or_else(|| Arc::new(MonotonicGate::new())))
        } else {
            None
        };

        let join = tokio::spawn(async move {
            let mut provider_handle = Some(handle);
            let mut notify_session_end = true;
            let mut reset_monotonic = false;
            loop {
                tokio::select! {
                    biased;
                    _ = stop_watch.changed() => {
                        if *stop_watch.borrow() {
                            if let Some(h) = provider_handle.take() { h.stop().await; }
                            break;
                        }
                    }
                    () = async {}, if *stop_watch.borrow() => {
                        if let Some(h) = provider_handle.take() { h.stop().await; }
                        break;
                    }
                    _ = &mut session_stop_rx => {
                        if let Some(h) = provider_handle.take() { h.stop().await; }
                        break;
                    }
                    maybe_u = prx.recv() => {
                        if let Some(u) = maybe_u {
                            if let Some(ref allowset) = allowed
                                && !allowset.contains(u.stream_symbol()) {
                                    #[cfg(feature = "tracing")]
                                    tracing::warn!(symbol = %u.stream_symbol(), provider_index = session_index, "dropping update for unassigned symbol");
                                    continue;
                                }

                            if enforce_monotonic {
                                let gate = monotonic_gate.as_ref().expect("monotonic gate must exist when enforcement enabled");
                                if !gate.allow(u.stream_symbol().as_str().to_string(), u.stream_ts()).await {
                                    #[cfg(feature = "tracing")]
                                    tracing::warn!(symbol = %u.stream_symbol(), ts = %u.stream_ts(), provider_index = session_index, "dropping out-of-order stream update (monotonic)");
                                    continue;
                                }
                            }

                            if tx_out.send(u).await.is_err() {
                                // Downstream dropped
                                notify_session_end = false;
                                if let Some(h) = provider_handle.take() { h.stop().await; }
                                break;
                            }
                        } else {
                            reset_monotonic = true;
                            if let Some(h) = provider_handle.take() { h.stop().await; }
                            break;
                        }
                    }
                }
            }

            if enforce_monotonic
                && reset_monotonic
                && let Some(gate) = &monotonic_gate
            {
                gate.reset_symbols(session_symbols.iter()).await;
            }

            if notify_session_end {
                let _ = event_tx.send((session_index, Arc::clone(&session_symbols)));
            }
        });

        SpawnedSession {
            join,
            stop_tx: Some(session_stop_tx),
        }
    }
}
