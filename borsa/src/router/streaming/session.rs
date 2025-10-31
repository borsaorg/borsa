use std::collections::HashSet;
use std::sync::Arc;

use borsa_core::Symbol;
use borsa_core::{QuoteUpdate, stream::StreamHandle};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;

use super::filters::MonotonicGate;

pub struct SpawnedSession {
    pub join: JoinHandle<()>,
    pub stop_tx: Option<oneshot::Sender<()>>,
}

pub struct SessionManager;

impl SessionManager {
    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        session_index: usize,
        handle: StreamHandle,
        mut prx: mpsc::Receiver<QuoteUpdate>,
        allowed: Option<HashSet<Symbol>>,
        mut stop_watch: watch::Receiver<bool>,
        enforce_monotonic: bool,
        monotonic_gate: Arc<MonotonicGate>,
        tx_out: mpsc::Sender<QuoteUpdate>,
        event_tx: tokio::sync::mpsc::UnboundedSender<(usize, Arc<[Symbol]>)>,
        session_symbols: Arc<[Symbol]>,
    ) -> SpawnedSession {
        let (session_stop_tx, mut session_stop_rx) = oneshot::channel::<()>();

        let join = tokio::spawn(async move {
            let mut provider_handle = Some(handle);
            let mut notify_session_end = true;
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
                    Ok(()) = &mut session_stop_rx => {
                        if let Some(h) = provider_handle.take() { h.stop().await; }
                        break;
                    }
                    maybe_u = prx.recv() => {
                        if let Some(u) = maybe_u {
                            if let Some(ref allowset) = allowed
                                && !allowset.contains(&u.symbol) {
                                    #[cfg(feature = "tracing")]
                                    tracing::warn!(symbol = %u.symbol, provider_index = session_index, "dropping update for unassigned symbol");
                                    continue;
                                }

                            if enforce_monotonic && !monotonic_gate.allow(&u).await {
                                #[cfg(feature = "tracing")]
                                tracing::warn!(symbol = %u.symbol, ts = %u.ts, provider_index = session_index, "dropping out-of-order stream update (monotonic)");
                                continue;
                            }

                            if tx_out.send(u).await.is_err() {
                                // Downstream dropped
                                notify_session_end = false;
                                if let Some(h) = provider_handle.take() { h.stop().await; }
                                break;
                            }
                        } else {
                            if let Some(h) = provider_handle.take() { h.stop().await; }
                            break;
                        }
                    }
                }
            }

            if notify_session_end {
                let _ = event_tx.send((session_index, session_symbols));
            }
        });

        SpawnedSession {
            join,
            stop_tx: Some(session_stop_tx),
        }
    }
}
