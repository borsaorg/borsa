use crate::router::streaming::{
    EligibleStreamProviders, KindSupervisorParams, collapse_stream_errors, spawn_kind_supervisor,
};
use crate::{BackoffConfig, Borsa};
use borsa_core::{
    AssetKind, BorsaConnector, BorsaError, Exchange, Instrument, QuoteUpdate, RoutingContext,
    stream::StreamHandle,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, watch};

impl Borsa {
    /// Start streaming quotes with automatic backoff and provider failover.
    ///
    /// Parameters:
    /// - `instruments`: list of instruments to stream (must be non-empty)
    /// - `backoff_override`: optional backoff settings; defaults to config or built-in
    ///
    /// Behavior and trade-offs:
    /// - Instruments are grouped by `AssetKind` and streamed via the first provider
    ///   that successfully connects per kind; on disconnect, a supervised loop
    ///   rotates to the next eligible provider with exponential backoff and jitter.
    /// - Jitter reduces synchronized reconnects (thundering herd) at the cost of
    ///   non-deterministic reconnect delay.
    /// - When multiple kinds are present, each kind runs independently and their
    ///   updates are fanned-in to a single channel.
    /// - The `allow` filter ensures only requested symbols are forwarded.
    /// - Stopping the returned `StreamHandle` terminates all supervised tasks.
    /// # Errors
    /// Returns an error if streaming initialization fails for all eligible providers of a kind
    /// or when no streaming-capable providers are available.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa::router::stream_quotes_with_backoff",
            skip(self, instruments, backoff_override)
        )
    )]
    #[allow(clippy::too_many_lines)]
    pub async fn stream_quotes_with_backoff(
        &self,
        instruments: &[Instrument],
        backoff_override: Option<BackoffConfig>,
    ) -> Result<(StreamHandle, mpsc::Receiver<QuoteUpdate>), BorsaError> {
        // Ensure this async function awaits at least once to avoid unused_async lint.
        tokio::task::yield_now().await;
        if instruments.is_empty() {
            return Err(borsa_core::BorsaError::InvalidArg(
                "instruments list cannot be empty".into(),
            ));
        }

        // Group instruments by (kind, exchange) to respect provider rules that depend on exchange.
        let mut by_group: HashMap<(AssetKind, Option<Exchange>), Vec<Instrument>> = HashMap::new();
        for inst in instruments.iter().cloned() {
            by_group
                .entry((*inst.kind(), inst.exchange().cloned()))
                .or_default()
                .push(inst);
        }

        let resolved_backoff: BackoffConfig =
            backoff_override.or(self.cfg.backoff).unwrap_or_default();

        // For each kind, spin up a supervisor loop identical to previous logic, then fan-in.
        let (tx, rx) = mpsc::channel::<QuoteUpdate>(1024);
        let (stop_tx, stop_rx) = oneshot::channel::<()>();
        let (stop_broadcast_tx, stop_broadcast_rx) = watch::channel(false);

        let mut joins = Vec::new();
        let mut init_receivers: Vec<oneshot::Receiver<Result<(), BorsaError>>> = Vec::new();
        for ((kind, ex), list) in by_group {
            let EligibleStreamProviders {
                providers,
                provider_symbols,
                union_symbols,
            } = self.eligible_stream_providers_for_context(kind, ex.as_ref(), &list)?;
            if union_symbols.is_empty() {
                continue;
            }

            // Strict policy failure precheck: find symbols requested but rejected by a strict rule.
            let requested: HashSet<String> = list.iter().map(|i| i.symbol().to_string()).collect();
            let rejected: Vec<String> = requested.difference(&union_symbols).cloned().collect();
            if !rejected.is_empty() {
                // Determine if strict rules excluded these symbols (vs capability absence).
                let mut strict_filtered: Vec<String> = Vec::new();
                // Consider only streaming-capable providers that support this kind
                let candidates: Vec<&Arc<dyn BorsaConnector>> = self
                    .connectors
                    .iter()
                    .filter(|c| c.as_stream_provider().is_some() && c.supports_kind(kind))
                    .collect();
                for sym in &rejected {
                    if !candidates.is_empty() {
                        let mut any_allowed = false;
                        for c in &candidates {
                            let ctx =
                                RoutingContext::new(Some(sym.as_str()), Some(kind), ex.clone());
                            if self
                                .cfg
                                .routing_policy
                                .providers
                                .provider_rank(&ctx, &c.key())
                                .is_some()
                            {
                                any_allowed = true;
                                break;
                            }
                        }
                        if !any_allowed {
                            strict_filtered.push(sym.clone());
                        }
                    }
                }
                if !strict_filtered.is_empty() {
                    return Err(BorsaError::StrictSymbolsRejected {
                        rejected: strict_filtered,
                    });
                }
            }

            // Decide mode: if any symbol in this group has an explicit provider preference
            // (rank != usize::MAX), route per-symbol; otherwise, use group-level fallback.
            let mut group_has_explicit: bool = false;
            'outer: for inst in &list {
                let sym = inst.symbol_str();
                for p in &providers {
                    let ctx = RoutingContext::new(Some(sym), Some(kind), ex.clone());
                    if let Some((rank, _)) = self
                        .cfg
                        .routing_policy
                        .providers
                        .provider_rank(&ctx, &p.key())
                        && rank != usize::MAX
                    {
                        group_has_explicit = true;
                        break 'outer;
                    }
                }
            }

            if group_has_explicit {
                // Partition symbols: those with explicit preferences get individual supervisors,
                // those without get grouped via the fallback supervisor.
                let mut symbols_with_explicit: Vec<String> = Vec::new();
                let mut symbols_without_explicit: HashSet<String> = HashSet::new();

                for sym in &requested {
                    if !union_symbols.contains(sym) {
                        // Symbol not eligible under current policy: do not start a stream for it.
                        continue;
                    }
                    // Gather candidate providers that allow this symbol
                    let mut candidates: Vec<(usize, Arc<dyn BorsaConnector>, usize)> = Vec::new();
                    let mut has_explicit_preference_for_sym = false;
                    for (idx, p) in providers.iter().cloned().enumerate() {
                        if !provider_symbols
                            .get(idx)
                            .is_some_and(|set| set.contains(sym.as_str()))
                        {
                            continue;
                        }
                        let ctx = RoutingContext::new(Some(sym.as_str()), Some(kind), ex.clone());
                        if let Some((rank, _strict)) = self
                            .cfg
                            .routing_policy
                            .providers
                            .provider_rank(&ctx, &p.key())
                        {
                            // store (rank, provider, providers_index) for tie-breaks
                            candidates.push((rank, p, idx));
                            if rank != usize::MAX {
                                has_explicit_preference_for_sym = true;
                            }
                        }
                    }

                    if candidates.is_empty() {
                        continue;
                    }
                    if has_explicit_preference_for_sym {
                        symbols_with_explicit.push(sym.clone());
                    } else {
                        symbols_without_explicit.insert(sym.clone());
                    }
                }

                // Group symbols with explicit preferences by their primary (best-ranked) provider
                let mut primary_groups: HashMap<usize, Vec<String>> = HashMap::new();
                for sym in &symbols_with_explicit {
                    // Gather candidate providers that allow this symbol
                    let mut candidates: Vec<(usize, usize)> = Vec::new(); // (rank, providers_idx)
                    for (idx, p) in providers.iter().enumerate() {
                        if !provider_symbols
                            .get(idx)
                            .is_some_and(|set| set.contains(sym.as_str()))
                        {
                            continue;
                        }
                        let ctx = RoutingContext::new(Some(sym.as_str()), Some(kind), ex.clone());
                        if let Some((rank, _strict)) = self
                            .cfg
                            .routing_policy
                            .providers
                            .provider_rank(&ctx, &p.key())
                        {
                            candidates.push((rank, idx));
                        }
                    }
                    if candidates.is_empty() {
                        continue;
                    }
                    candidates.sort_by_key(|(rank, providers_idx)| (*rank, *providers_idx));
                    let (_best_rank, best_idx) = candidates[0];
                    primary_groups
                        .entry(best_idx)
                        .or_default()
                        .push(sym.clone());
                }

                // For each primary provider group, spawn a single supervisor that multiplexes all its symbols
                for (primary_idx, group_syms) in primary_groups {
                    let group_syms_set: HashSet<String> = group_syms.iter().cloned().collect();

                    // Build provider chain starting with the primary, followed by the rest in stable order
                    let mut chain_indices: Vec<usize> = Vec::with_capacity(providers.len());
                    chain_indices.push(primary_idx);
                    for j in 0..providers.len() {
                        if j != primary_idx {
                            chain_indices.push(j);
                        }
                    }

                    let mut chain_providers: Vec<Arc<dyn BorsaConnector>> =
                        Vec::with_capacity(chain_indices.len());
                    let mut provider_instruments: Vec<Vec<Instrument>> =
                        Vec::with_capacity(chain_indices.len());
                    let mut provider_allow: Vec<HashSet<String>> =
                        Vec::with_capacity(chain_indices.len());

                    for &orig_idx in &chain_indices {
                        chain_providers.push(providers[orig_idx].clone());
                        let allow_full =
                            provider_symbols.get(orig_idx).cloned().unwrap_or_default();
                        let filtered_allow: HashSet<String> = allow_full
                            .into_iter()
                            .filter(|s| group_syms_set.contains(s))
                            .collect();
                        let assigned = list
                            .iter()
                            .filter(|&inst| filtered_allow.contains(inst.symbol().as_str()))
                            .cloned()
                            .collect::<Vec<_>>();
                        provider_instruments.push(assigned);
                        provider_allow.push(filtered_allow);
                    }

                    let min_backoff_ms = resolved_backoff.min_backoff_ms;
                    let max_backoff_ms = resolved_backoff.max_backoff_ms;
                    let factor = resolved_backoff.factor.max(1);
                    let jitter_percent = resolved_backoff.jitter_percent.min(100);

                    let required_symbols: HashSet<String> = group_syms_set.clone();
                    let (init_tx, init_rx) = oneshot::channel();
                    let params = KindSupervisorParams {
                        providers: chain_providers,
                        provider_instruments,
                        provider_allow,
                        required_symbols,
                        min_backoff_ms,
                        max_backoff_ms,
                        factor,
                        jitter_percent: u32::from(jitter_percent),
                        initial_notify: Some(init_tx),
                        enforce_monotonic: self.cfg.stream_enforce_monotonic_timestamps,
                    };
                    let join = spawn_kind_supervisor(params, stop_broadcast_rx.clone(), tx.clone());
                    joins.push(join);
                    init_receivers.push(init_rx);
                }

                // Handle symbols without explicit preferences via group-level supervisor
                if !symbols_without_explicit.is_empty() {
                    let mut provider_instruments: Vec<Vec<Instrument>> =
                        Vec::with_capacity(providers.len());
                    let mut provider_allow: Vec<HashSet<String>> =
                        Vec::with_capacity(providers.len());
                    for allow in &provider_symbols {
                        let assigned = list
                            .iter()
                            .filter(|&inst| {
                                allow.contains(inst.symbol().as_str())
                                    && symbols_without_explicit.contains(inst.symbol().as_str())
                            })
                            .cloned()
                            .collect::<Vec<_>>();
                        provider_instruments.push(assigned);
                        let filtered_allow: HashSet<String> = allow
                            .iter()
                            .filter(|s| symbols_without_explicit.contains(s.as_str()))
                            .cloned()
                            .collect();
                        provider_allow.push(filtered_allow);
                    }

                    let min_backoff_ms = resolved_backoff.min_backoff_ms;
                    let max_backoff_ms = resolved_backoff.max_backoff_ms;
                    let factor = resolved_backoff.factor.max(1);
                    let jitter_percent = resolved_backoff.jitter_percent.min(100);

                    let (init_tx, init_rx) = oneshot::channel();

                    let params = KindSupervisorParams {
                        providers: providers.clone(),
                        provider_instruments,
                        provider_allow,
                        required_symbols: symbols_without_explicit.clone(),
                        min_backoff_ms,
                        max_backoff_ms,
                        factor,
                        jitter_percent: u32::from(jitter_percent),
                        initial_notify: Some(init_tx),
                        enforce_monotonic: self.cfg.stream_enforce_monotonic_timestamps,
                    };
                    let join = spawn_kind_supervisor(params, stop_broadcast_rx.clone(), tx.clone());
                    joins.push(join);
                    init_receivers.push(init_rx);
                }
            } else {
                // No explicit preferences for this group: use group-level supervisor (fallback allowed)
                let mut provider_instruments: Vec<Vec<Instrument>> =
                    Vec::with_capacity(providers.len());
                let mut provider_allow: Vec<HashSet<String>> = Vec::with_capacity(providers.len());
                for allow in &provider_symbols {
                    let assigned = list
                        .iter()
                        .filter(|&inst| allow.contains(inst.symbol().as_str()))
                        .cloned()
                        .collect::<Vec<_>>();
                    provider_instruments.push(assigned);
                    provider_allow.push(allow.clone());
                }

                let min_backoff_ms = resolved_backoff.min_backoff_ms;
                let max_backoff_ms = resolved_backoff.max_backoff_ms;
                let factor = resolved_backoff.factor.max(1);
                let jitter_percent = resolved_backoff.jitter_percent.min(100);

                let (init_tx, init_rx) = oneshot::channel();

                let required_symbols: HashSet<String> = list
                    .iter()
                    .filter(|inst| union_symbols.contains(inst.symbol().as_str()))
                    .map(|inst| inst.symbol().to_string())
                    .collect();

                let params = KindSupervisorParams {
                    providers,
                    provider_instruments,
                    provider_allow,
                    required_symbols,
                    min_backoff_ms,
                    max_backoff_ms,
                    factor,
                    jitter_percent: u32::from(jitter_percent),
                    initial_notify: Some(init_tx),
                    enforce_monotonic: self.cfg.stream_enforce_monotonic_timestamps,
                };
                let join = spawn_kind_supervisor(params, stop_broadcast_rx.clone(), tx.clone());
                joins.push(join);
                init_receivers.push(init_rx);
            }
        }

        // Ensure at least one kind connected successfully before returning a handle.
        let mut init_errors: Vec<BorsaError> = Vec::new();
        let mut success_kinds: usize = 0;
        for rx in init_receivers {
            match rx.await {
                Ok(Ok(())) => {
                    success_kinds += 1;
                }
                Ok(Err(e)) => init_errors.push(e),
                Err(_) => init_errors.push(BorsaError::Other(
                    "stream supervisor dropped before initialization".into(),
                )),
            }
        }

        if success_kinds == 0 || !init_errors.is_empty() {
            let err = collapse_stream_errors(init_errors);
            for join in joins {
                join.abort();
            }
            let _ = stop_broadcast_tx.send(true);
            return Err(err);
        }

        // Supervisor to await stop signal OR downstream closure and then stop all children
        let supervisor = tokio::spawn(async move {
            let mut stop_rx_inner = stop_rx;
            tokio::select! {
                _ = &mut stop_rx_inner => {}
                // Downstream receiver dropped for the fan-in channel
                () = async {}, if tx.is_closed() => {}
            }
            let _ = stop_broadcast_tx.send(true);
            for j in joins {
                let _ = j.await;
            }
        });

        Ok((StreamHandle::new(supervisor, stop_tx), rx))
    }

    /// Start streaming quotes using the configured backoff settings.
    ///
    /// Notes:
    /// - Convenience wrapper around `stream_quotes_with_backoff` using the builder
    ///   configuration (or defaults) for backoff.
    /// # Errors
    /// Returns an error if streaming initialization fails for all providers.
    pub async fn stream_quotes(
        &self,
        instruments: &[Instrument],
    ) -> Result<(StreamHandle, mpsc::Receiver<QuoteUpdate>), BorsaError> {
        self.stream_quotes_with_backoff(instruments, None).await
    }
}

impl Borsa {
    #[cfg(any())]
    #[allow(clippy::too_many_lines)]
    fn spawn_kind_supervisor(
        params: KindSupervisorParams,
        mut stop_watch: watch::Receiver<bool>,
        tx_clone: mpsc::Sender<QuoteUpdate>,
    ) -> JoinHandle<()> {
        #[derive(Debug)]
        enum ProviderEvent {
            SessionEnded {
                provider_index: usize,
                symbols: Arc<[String]>,
            },
        }

        struct ActiveSession {
            join: JoinHandle<()>,
            symbols: Arc<[String]>,
            stop_tx: Option<oneshot::Sender<()>>,
        }

        tokio::spawn(async move {
            use std::collections::hash_map::Entry;
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
            let mut coverage_counts: HashMap<String, usize> = HashMap::new();
            let mut active_sessions: Vec<Option<ActiveSession>> =
                Vec::with_capacity(providers.len());
            active_sessions.resize_with(providers.len(), || None);
            let last_ts_by_symbol = Arc::new(Mutex::new(HashMap::new()));
            let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<ProviderEvent>();
            // When a session ends for provider `p`, we skip attempting `p` until after
            // one backoff sleep completes. This allows immediate failover to other
            // providers without delaying reconnection attempts to `p`.
            let mut cooldown_provider: Option<usize> = None;

            loop {
                // If downstream receiver is gone, terminate supervisor and all sessions
                if tx_clone.is_closed() {
                    for session in &mut active_sessions {
                        if let Some(ActiveSession { join, .. }) = session.take() {
                            let _ = join.await;
                        }
                    }
                    return;
                }
                let mut connected_this_round = false;
                let mut attempted_reconnect_this_round = false;

                for offset in 0..providers.len() {
                    let i = (start_index + offset) % providers.len();

                    if active_sessions.get(i).and_then(|s| s.as_ref()).is_some() {
                        continue;
                    }

                    if cooldown_provider.is_some() && cooldown_provider == Some(i) {
                        continue;
                    }

                    let Some(sp) = providers[i].as_stream_provider() else {
                        continue;
                    };

                    let provider_symbols = provider_allow.get(i);
                    let provider_insts = provider_instruments.get(i);
                    let needed_from_provider: Vec<Instrument> =
                        match (provider_symbols, provider_insts) {
                            (Some(allow_set), Some(insts)) => insts
                                .iter()
                                .filter(|inst| {
                                    let sym = inst.symbol().as_str();
                                    if !allow_set.contains(sym) || !required_symbols.contains(sym) {
                                        return false;
                                    }
                                    let already_covered =
                                        coverage_counts.get(sym).copied().unwrap_or(0) > 0;
                                    if !already_covered {
                                        // Fill gaps first
                                        return true;
                                    }
                                    // Upgrade policy: if symbol is currently only covered by lower-priority
                                    // sessions, attempt to connect this higher-priority provider `i` and later
                                    // preempt overlapping lower-priority sessions.
                                    !active_sessions.iter().enumerate().any(|(j, s)| {
                                        j < i
                                            && s.as_ref().is_some_and(|sess| {
                                                sess.symbols.iter().any(|s2| s2 == sym)
                                            })
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
                        Ok((handle, mut prx)) => {
                            connected_this_round = true;
                            if let Some(tx) = initial_notify.take() {
                                let _ = tx.send(Ok(()));
                            }
                            initial_errors.clear();

                            let symbols_vec: Vec<String> = needed_from_provider
                                .iter()
                                .map(|inst| inst.symbol().as_str().to_string())
                                .collect();
                            let symbols_arc: Arc<[String]> =
                                Arc::from(symbols_vec.into_boxed_slice());
                            for sym in symbols_arc.iter() {
                                *coverage_counts.entry(sym.clone()).or_insert(0) += 1;
                            }

                            let allowed = provider_allow.get(i).cloned();
                            let session_symbols = Arc::clone(&symbols_arc);
                            let event_tx_clone = event_tx.clone();
                            let tx_out = tx_clone.clone();
                            let mut stop_watch_clone = stop_watch.clone();
                            let last_ts = Arc::clone(&last_ts_by_symbol);
                            let session_index = i;
                            let (session_stop_tx, mut session_stop_rx) = oneshot::channel::<()>();

                            let join = tokio::spawn(async move {
                                let mut provider_handle = Some(handle);
                                let mut notify_session_end = true;
                                loop {
                                    tokio::select! {
                                        biased;
                                        _ = stop_watch_clone.changed() => {
                                            if *stop_watch_clone.borrow() {
                                                if let Some(h) = provider_handle.take() {
                                                    h.stop().await;
                                                }
                                                break;
                                            }
                                        }
                                        () = async {}, if *stop_watch_clone.borrow() => {
                                            if let Some(h) = provider_handle.take() {
                                                h.stop().await;
                                            }
                                            break;
                                        }
                                        Ok(()) = &mut session_stop_rx => {
                                            if let Some(h) = provider_handle.take() {
                                                h.stop().await;
                                            }
                                            break;
                                        }
                                        maybe_u = prx.recv() => {
                                            if let Some(u) = maybe_u {
                                                let mut pass = true;
                                                if let Some(ref allowset) = allowed
                                                    && !allowset.contains(u.symbol.as_str()) {
                                                        pass = false;
                                                        #[cfg(feature = "tracing")]
                                                        tracing::warn!(symbol = %u.symbol, provider_index = session_index, "dropping update for unassigned symbol");
                                                    }

                                                if pass && enforce_monotonic {
                                                    let sym = u.symbol.as_str().to_string();
                                                    let mut guard = last_ts.lock().await;
                                                    let mut older_than_last: Option<chrono::DateTime<chrono::Utc>> = None;
                                                    match guard.entry(sym.clone()) {
                                                        Entry::Occupied(mut entry) => {
                                                            let prev = *entry.get();
                                                            if u.ts < prev {
                                                                older_than_last = Some(prev);
                                                                pass = false;
                                                            } else if u.ts > prev {
                                                                *entry.get_mut() = u.ts;
                                                            }
                                                        }
                                                        Entry::Vacant(entry) => {
                                                            entry.insert(u.ts);
                                                        }
                                                    }
                                                    drop(guard);
                                                    if let Some(prev) = older_than_last {
                                                        #[cfg(feature = "tracing")]
                                                        tracing::warn!(symbol = %u.symbol, prev_ts = %prev, ts = %u.ts, provider_index = session_index, "dropping out-of-order stream update (ts older than last seen)");
                                                    }
                                                }

                                                if pass
                                                    && tx_out.send(u).await.is_err() {
                                                        // Downstream receiver dropped: terminate without notifying supervisor
                                                        notify_session_end = false;
                                                        if let Some(h) = provider_handle.take() {
                                                            h.stop().await;
                                                        }
                                                        break;
                                                    }
                                            } else {
                                                if let Some(h) = provider_handle.take() {
                                                    h.stop().await;
                                                }
                                                break;
                                            }
                                        }
                                    }
                                }

                                if notify_session_end {
                                    let _ = event_tx_clone.send(ProviderEvent::SessionEnded {
                                        provider_index: session_index,
                                        symbols: session_symbols,
                                    });
                                }
                            });

                            active_sessions[i] = Some(ActiveSession {
                                join,
                                symbols: Arc::clone(&symbols_arc),
                                stop_tx: Some(session_stop_tx),
                            });
                            start_index = (i + 1) % providers.len();

                            // Preempt lower-priority sessions that overlap on any of these symbols
                            for j in (i + 1)..providers.len() {
                                if let Some(sess) =
                                    active_sessions.get_mut(j).and_then(|s| s.as_mut())
                                {
                                    let overlaps = sess
                                        .symbols
                                        .iter()
                                        .any(|s| symbols_arc.iter().any(|t| t == s));
                                    if overlaps && let Some(tx) = sess.stop_tx.take() {
                                        let _ = tx.send(());
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            if initial_notify.is_some() {
                                initial_errors.push(crate::core::tag_err(providers[i].name(), err));
                            }
                        }
                    }
                }

                let base_ms = backoff_ms;
                let wait_ms = jitter_wait(base_ms, jitter_percent);

                let mut woke_by_sleep: bool = false;
                tokio::select! {
                    _ = stop_watch.changed() => {
                        if *stop_watch.borrow() {
                            for session in &mut active_sessions {
                                if let Some(ActiveSession { join, .. }) = session.take() {
                                    let _ = join.await;
                                }
                            }
                            return;
                        }
                    }
                    () = async {}, if *stop_watch.borrow() => {
                        for session in &mut active_sessions {
                            if let Some(ActiveSession { join, .. }) = session.take() {
                                let _ = join.await;
                            }
                        }
                        return;
                    }
                    // Downstream receiver dropped: terminate supervisor and all sessions
                    () = async {}, if tx_clone.is_closed() => {
                        for session in &mut active_sessions {
                            if let Some(ActiveSession { join, .. }) = session.take() {
                                let _ = join.await;
                            }
                        }
                        return;
                    }
                    Some(event) = event_rx.recv() => {
                        match event {
                            ProviderEvent::SessionEnded { provider_index, symbols } => {
                                if let Some(ActiveSession { join, .. }) = active_sessions
                                    .get_mut(provider_index)
                                    .and_then(std::option::Option::take)
                                {
                                    let _ = join.await;
                                }
                                for sym in symbols.iter() {
                                    if let Entry::Occupied(mut entry) = coverage_counts.entry(sym.clone()) {
                                        if *entry.get() > 1 {
                                            *entry.get_mut() -= 1;
                                        } else {
                                            entry.remove();
                                        }
                                    }
                                }
                                // Enforce a backoff delay before attempting reconnection to this provider
                                cooldown_provider = Some(provider_index);
                            }
                        }
                    }
                    () = sleep(Duration::from_millis(wait_ms)) => { woke_by_sleep = true; cooldown_provider = None; }
                }

                if connected_this_round {
                    backoff_ms = min_backoff_ms;
                } else if woke_by_sleep && attempted_reconnect_this_round {
                    if active_sessions.iter().all(std::option::Option::is_none) {
                        if let Some(tx) = initial_notify.take() {
                            let err = collapse_stream_errors(std::mem::take(&mut initial_errors));
                            let _ = tx.send(Err(err));
                            return;
                        }
                        backoff_ms = std::cmp::min(
                            max_backoff_ms,
                            base_ms.saturating_mul(u64::from(factor)),
                        );
                        start_index = 0;
                    } else {
                        backoff_ms = std::cmp::min(
                            max_backoff_ms,
                            base_ms.saturating_mul(u64::from(factor)),
                        );
                    }
                }
            }
        })
    }
}
