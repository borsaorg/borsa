use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use borsa_core::{BorsaConnector, BorsaError, Instrument, QuoteUpdate, Symbol};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;

use super::backoff::jitter_wait;
use super::error::collapse_stream_errors;
use super::filters::MonotonicGate;
use super::session::SessionManager;

pub struct KindSupervisorParams {
    pub providers: Vec<Arc<dyn BorsaConnector>>,
    /// Assigned instruments per provider, aligned by index with `providers`.
    pub provider_instruments: Vec<Vec<Instrument>>,
    /// Allowed symbol set per provider, aligned by index with `providers`.
    pub provider_allow: Vec<HashSet<Symbol>>,
    /// Full set of symbols that must be covered across all providers.
    pub required_symbols: HashSet<Symbol>,
    pub min_backoff_ms: u64,
    pub max_backoff_ms: u64,
    pub factor: u32,
    pub jitter_percent: u32,
    pub initial_notify: Option<oneshot::Sender<Result<(), BorsaError>>>,
    pub enforce_monotonic: bool,
}

#[allow(clippy::too_many_lines)]
pub fn spawn_kind_supervisor(
    params: KindSupervisorParams,
    mut stop_watch: watch::Receiver<bool>,
    tx_clone: mpsc::Sender<QuoteUpdate>,
) -> JoinHandle<()> {
    struct ActiveSession {
        join: JoinHandle<()>,
        stop_tx: Option<oneshot::Sender<()>>,
    }

    tokio::spawn(async move {
        use super::supervisor_sm as sm;
        use std::pin::Pin;
        use tokio::time::Duration;

        type StartFuture = Pin<
            Box<
                dyn core::future::Future<
                        Output = (
                            usize,
                            Result<
                                (
                                    borsa_core::stream::StreamHandle,
                                    tokio::sync::mpsc::Receiver<QuoteUpdate>,
                                    Arc<[Symbol]>,
                                ),
                                BorsaError,
                            >,
                        ),
                    > + Send,
            >,
        >;

        let KindSupervisorParams {
            providers,
            provider_instruments,
            provider_allow,
            required_symbols,
            min_backoff_ms,
            max_backoff_ms,
            factor,
            jitter_percent,
            mut initial_notify,
            enforce_monotonic,
        } = params;

        if providers.is_empty() {
            if let Some(tx) = initial_notify.take() {
                let err = collapse_stream_errors(Vec::new());
                let _ = tx.send(Err(err));
            }
            return;
        }

        let providers_can_stream: Vec<bool> = providers
            .iter()
            .map(|p| p.as_stream_provider().is_some())
            .collect();

        let mut supervisor = sm::Supervisor {
            providers: vec![sm::ProviderState::Idle; providers.len()],
            provider_instruments,
            provider_allow,
            required_symbols,
            providers_can_stream,
            start_index: 0,
            scan_cursor: 0,
            round_exhausted: false,
            backoff_ms: min_backoff_ms,
            min_backoff_ms,
            max_backoff_ms,
            factor,
            attempted_since_last_tick: false,
            phase: sm::Phase::Startup {
                initial_tx: initial_notify.take(),
                accumulated_errors: Vec::new(),
            },
        };

        let monotonic_gate = Arc::new(MonotonicGate::new());
        let (event_tx, mut event_rx) =
            tokio::sync::mpsc::unbounded_channel::<(usize, Arc<[Symbol]>)>();

        let mut session_tasks: HashMap<usize, ActiveSession> = HashMap::new();
        let mut in_flight_start: Option<StartFuture> = None;
        let mut backoff_timer: Option<Pin<Box<tokio::time::Sleep>>> =
            Some(Box::pin(tokio::time::sleep(Duration::from_millis(
                jitter_wait(supervisor.current_delay_ms(), jitter_percent),
            ))));

        // Kick off initial start attempts proactively before the first poll
        if supervisor.should_attempt_starts() {
            let initial_actions = supervisor.compute_needed_starts();
            for action in initial_actions {
                if let sm::Action::RequestStart { id, instruments } = action
                    && in_flight_start.is_none()
                {
                    let provider = Arc::clone(&providers[id]);
                    let syms: Arc<[Symbol]> = Arc::from(
                        instruments
                            .iter()
                            .map(|inst| inst.symbol().clone())
                            .collect::<Vec<_>>()
                            .into_boxed_slice(),
                    );
                    let fut = async move {
                        let provider_name = provider.name();
                        let res = match provider.as_stream_provider() {
                            Some(sp) => match sp.stream_quotes(&instruments).await {
                                Ok((handle, prx)) => Ok((handle, prx, syms)),
                                Err(err) => Err(crate::core::tag_err(provider_name, err)),
                            },
                            None => Err(BorsaError::unsupported("stream_quotes")),
                        };
                        (id, res)
                    };
                    in_flight_start = Some(Box::pin(fut));
                }
            }
        }

        loop {
            let event = tokio::select! {
                _ = stop_watch.changed() => sm::Event::Shutdown,
                () = async {}, if *stop_watch.borrow() => sm::Event::Shutdown,
                () = tx_clone.closed() => sm::Event::DownstreamClosed,
                Some((id, syms)) = event_rx.recv() => sm::Event::SessionEnded { id, symbols: syms },
                outcome = async { in_flight_start.as_mut().unwrap().await }, if in_flight_start.is_some() => {
                    // Map outcome to Event::ProviderStartSucceeded/Failed
                    in_flight_start = None;
                    let (id, res) = outcome;
                    match res {
                        Ok((handle, prx, symbols)) => {
                            let allowed = supervisor.provider_allow.get(id).cloned();
                            let spawned = SessionManager::spawn(
                                id,
                                handle,
                                prx,
                                allowed,
                                stop_watch.clone(),
                                enforce_monotonic,
                                Arc::clone(&monotonic_gate),
                                tx_clone.clone(),
                                event_tx.clone(),
                                Arc::clone(&symbols),
                            );
                            session_tasks.insert(id, ActiveSession { join: spawned.join, stop_tx: spawned.stop_tx });
                            sm::Event::ProviderStartSucceeded { id, symbols }
                        }
                        Err(e) => {
                            sm::Event::ProviderStartFailed { id, error: e }
                        }
                    }
                }
                () = async { backoff_timer.as_mut().unwrap().await }, if backoff_timer.is_some() => sm::Event::BackoffTick,
            };

            let (new_sm, actions) = supervisor.handle(event);
            supervisor = new_sm;

            for action in actions {
                match action {
                    sm::Action::RequestStart { id, instruments } => {
                        if in_flight_start.is_none() {
                            let provider = Arc::clone(&providers[id]);
                            let syms: Arc<[Symbol]> = Arc::from(
                                instruments
                                    .iter()
                                    .map(|inst| inst.symbol().clone())
                                    .collect::<Vec<_>>()
                                    .into_boxed_slice(),
                            );
                            let fut = async move {
                                let provider_name = provider.name();
                                let res = match provider.as_stream_provider() {
                                    Some(sp) => match sp.stream_quotes(&instruments).await {
                                        Ok((handle, prx)) => Ok((handle, prx, syms)),
                                        Err(err) => Err(crate::core::tag_err(provider_name, err)),
                                    },
                                    None => Err(BorsaError::unsupported("stream_quotes")),
                                };
                                (id, res)
                            };
                            in_flight_start = Some(Box::pin(fut));
                        }
                    }
                    sm::Action::StopAll => {
                        for sess in session_tasks.values_mut() {
                            if let Some(tx) = sess.stop_tx.take() {
                                let _ = tx.send(());
                            }
                        }
                    }
                    sm::Action::AwaitAll => {
                        for (_id, sess) in session_tasks.drain() {
                            let _ = sess.join.await;
                        }
                        return;
                    }
                    sm::Action::NotifyInitial { tx, result } => {
                        let _ = tx.send(result);
                        if matches!(supervisor.phase, sm::Phase::Terminated) {
                            return;
                        }
                    }
                    sm::Action::ScheduleBackoffTick { delay_ms } => {
                        backoff_timer = Some(Box::pin(tokio::time::sleep(Duration::from_millis(
                            jitter_wait(delay_ms, jitter_percent),
                        ))));
                    }
                    sm::Action::PreemptSessions { provider_ids } => {
                        for id in provider_ids {
                            if let Some(sess) = session_tasks.get_mut(&id)
                                && let Some(tx) = sess.stop_tx.take()
                            {
                                #[cfg(feature = "tracing")]
                                tracing::info!(
                                    preempted_index = id,
                                    "preempting lower-priority overlapping session"
                                );
                                let _ = tx.send(());
                            }
                        }
                    }
                }
            }
        }
    })
}
