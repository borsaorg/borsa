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
        symbols: Arc<[Symbol]>,
        stop_tx: Option<oneshot::Sender<()>>,
    }

    tokio::spawn(async move {
        use tokio::time::{Duration, sleep};

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

        let mut start_index: usize = 0;
        let mut backoff_ms: u64 = min_backoff_ms;
        let mut initial_errors: Vec<BorsaError> = Vec::new();
        let mut coverage_counts: HashMap<Symbol, usize> = HashMap::new();
        let mut active_sessions: Vec<Option<ActiveSession>> = Vec::with_capacity(providers.len());
        active_sessions.resize_with(providers.len(), || None);
        let monotonic_gate = Arc::new(MonotonicGate::new());
        let (event_tx, mut event_rx) =
            tokio::sync::mpsc::unbounded_channel::<(usize, Arc<[Symbol]>)>();
        let mut cooldown_providers: HashSet<usize> = HashSet::new();

        loop {
            if tx_clone.is_closed() {
                // Signal all sessions to stop before awaiting their termination
                for session in &mut active_sessions {
                    if let Some(sess) = session.as_mut()
                        && let Some(tx) = sess.stop_tx.take()
                    {
                        let _ = tx.send(());
                    }
                }
                for session in &mut active_sessions {
                    if let Some(ActiveSession { join, .. }) = session.take() {
                        let _ = join.await;
                    }
                }
                return;
            }

            let mut reconnected_from_cooldown = false;
            let mut attempted_reconnect_this_round = false;
            let cooldown_snapshot = cooldown_providers.clone();

            for offset in 0..providers.len() {
                let i = (start_index + offset) % providers.len();

                if active_sessions.get(i).and_then(|s| s.as_ref()).is_some() {
                    continue;
                }

                if cooldown_providers.contains(&i) {
                    continue;
                }

                let Some(sp) = providers[i].as_stream_provider() else {
                    continue;
                };

                let provider_symbols = provider_allow.get(i);
                let provider_insts = provider_instruments.get(i);
                let needed_from_provider: Vec<Instrument> = match (provider_symbols, provider_insts)
                {
                    (Some(allow_set), Some(insts)) => insts
                        .iter()
                        .filter(|inst| {
                            let sym = inst.symbol();
                            if !allow_set.contains(sym) || !required_symbols.contains(sym) {
                                return false;
                            }
                            let already_covered =
                                coverage_counts.get(sym).copied().unwrap_or(0) > 0;
                            if !already_covered {
                                return true;
                            }
                            !active_sessions.iter().enumerate().any(|(j, s)| {
                                j < i
                                    && s.as_ref()
                                        .is_some_and(|sess| sess.symbols.iter().any(|s2| s2 == sym))
                            })
                        })
                        .cloned()
                        .collect(),
                    _ => Vec::new(),
                };

                if needed_from_provider.is_empty() {
                    continue;
                }

                attempted_reconnect_this_round = true;
                match sp.stream_quotes(&needed_from_provider).await {
                    Ok((handle, prx)) => {
                        if cooldown_snapshot.contains(&i) {
                            reconnected_from_cooldown = true;
                        }
                        #[cfg(feature = "tracing")]
                        tracing::info!(provider_index = i, symbols = ?needed_from_provider.iter().map(|x| x.symbol().to_string()).collect::<Vec<_>>(), "stream session started");
                        if let Some(tx) = initial_notify.take() {
                            let _ = tx.send(Ok(()));
                        }
                        initial_errors.clear();

                        let symbols_vec: Vec<Symbol> = needed_from_provider
                            .iter()
                            .map(|inst| inst.symbol().clone())
                            .collect();
                        let symbols_arc: Arc<[Symbol]> = Arc::from(symbols_vec.into_boxed_slice());
                        for sym in symbols_arc.iter() {
                            *coverage_counts.entry(sym.clone()).or_insert(0) += 1;
                        }

                        let allowed = provider_allow.get(i).cloned();
                        let spawned = SessionManager::spawn(
                            i,
                            handle,
                            prx,
                            allowed,
                            stop_watch.clone(),
                            enforce_monotonic,
                            Arc::clone(&monotonic_gate),
                            tx_clone.clone(),
                            event_tx.clone(),
                            Arc::clone(&symbols_arc),
                        );

                        active_sessions[i] = Some(ActiveSession {
                            join: spawned.join,
                            symbols: Arc::clone(&symbols_arc),
                            stop_tx: spawned.stop_tx,
                        });
                        start_index = (i + 1) % providers.len();

                        // Preempt lower-priority sessions that overlap on any of these symbols
                        for j in (i + 1)..providers.len() {
                            if let Some(sess) = active_sessions.get_mut(j).and_then(|s| s.as_mut())
                            {
                                let overlaps = sess
                                    .symbols
                                    .iter()
                                    .any(|s| symbols_arc.iter().any(|t| t == s));
                                if overlaps && let Some(tx) = sess.stop_tx.take() {
                                    #[cfg(feature = "tracing")]
                                    tracing::info!(
                                        preempted_index = j,
                                        by_index = i,
                                        "preempting lower-priority overlapping session"
                                    );
                                    let _ = tx.send(());
                                }
                            }
                        }
                    }
                    Err(err) => {
                        if initial_notify.is_some() {
                            initial_errors.push(crate::core::tag_err(providers[i].name(), err));
                        }
                        #[cfg(feature = "tracing")]
                        {
                            let err_str = initial_errors.last().map_or_else(
                                || "unknown".to_string(),
                                std::string::ToString::to_string,
                            );
                            tracing::warn!(
                                provider_index = i,
                                error = %err_str,
                                "stream session failed to start"
                            );
                        }
                    }
                }
            }

            let base_ms = backoff_ms;
            let wait_ms = jitter_wait(base_ms, jitter_percent);

            let mut woke_by_sleep: bool = false;

            // Always use a timer to periodically re-evaluate provider priorities.
            // This enables failback to higher-priority providers and clears cooldown.
            tokio::select! {
                _ = stop_watch.changed() => {
                    if *stop_watch.borrow() {
                        // Signal all sessions to stop before awaiting their termination
                        for session in &mut active_sessions { if let Some(sess) = session.as_mut() && let Some(tx) = sess.stop_tx.take() { let _ = tx.send(()); } }
                        for session in &mut active_sessions { if let Some(ActiveSession { join, .. }) = session.take() { let _ = join.await; } }
                        return;
                    }
                }
                () = async {}, if *stop_watch.borrow() => {
                    // Signal all sessions to stop before awaiting their termination
                    for session in &mut active_sessions { if let Some(sess) = session.as_mut() && let Some(tx) = sess.stop_tx.take() { let _ = tx.send(()); } }
                    for session in &mut active_sessions { if let Some(ActiveSession { join, .. }) = session.take() { let _ = join.await; } }
                    return;
                }
                () = async {}, if tx_clone.is_closed() => {
                    // Signal all sessions to stop before awaiting their termination
                    for session in &mut active_sessions { if let Some(sess) = session.as_mut() && let Some(tx) = sess.stop_tx.take() { let _ = tx.send(()); } }
                    for session in &mut active_sessions { if let Some(ActiveSession { join, .. }) = session.take() { let _ = join.await; } }
                    return;
                }
                Some((provider_index, symbols)) = event_rx.recv() => {
                    #[cfg(feature = "tracing")]
                    tracing::info!(provider_index, symbols = ?symbols, "stream session ended");
                    if let Some(ActiveSession { join, .. }) = active_sessions.get_mut(provider_index).and_then(std::option::Option::take) { let _ = join.await; }
                    for sym in symbols.iter() {
                        use std::collections::hash_map::Entry;
                        if let Entry::Occupied(mut entry) = coverage_counts.entry(sym.clone()) {
                            if *entry.get() > 1 { *entry.get_mut() -= 1; } else { entry.remove(); }
                        }
                    }
                    cooldown_providers.insert(provider_index);
                    #[cfg(feature = "tracing")]
                    tracing::debug!(provider_index, "cooldown set for provider after session end");
                }
                () = sleep(Duration::from_millis(wait_ms)) => { woke_by_sleep = true; cooldown_providers.clear(); #[cfg(feature = "tracing")] tracing::debug!(wait_ms, "backoff tick"); }
            }

            if reconnected_from_cooldown {
                backoff_ms = min_backoff_ms;
                start_index = 0;
            } else if woke_by_sleep && attempted_reconnect_this_round {
                if active_sessions.iter().all(std::option::Option::is_none) {
                    if let Some(tx) = initial_notify.take() {
                        let err = collapse_stream_errors(std::mem::take(&mut initial_errors));
                        let _ = tx.send(Err(err));
                        return;
                    }
                    backoff_ms =
                        std::cmp::min(max_backoff_ms, base_ms.saturating_mul(u64::from(factor)));
                    start_index = 0;
                } else {
                    backoff_ms =
                        std::cmp::min(max_backoff_ms, base_ms.saturating_mul(u64::from(factor)));
                }
            }
        }
    })
}
