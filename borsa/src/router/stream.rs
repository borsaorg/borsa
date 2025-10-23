use crate::{BackoffConfig, Borsa};
use borsa_core::{BorsaConnector, BorsaError, Instrument, RoutingContext};
use rand::Rng;
use std::collections::HashSet;

type StreamProviderScore = (
    usize,
    usize,
    std::sync::Arc<dyn BorsaConnector>,
    HashSet<String>,
);

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
    ) -> Result<
        (
            borsa_core::stream::StreamHandle,
            tokio::sync::mpsc::Receiver<borsa_core::QuoteUpdate>,
        ),
        borsa_core::BorsaError,
    > {
        // Ensure this async function awaits at least once to avoid unused_async lint.
        tokio::task::yield_now().await;
        if instruments.is_empty() {
            return Err(borsa_core::BorsaError::InvalidArg(
                "instruments list cannot be empty".into(),
            ));
        }

        // Group instruments by (kind, exchange) to respect provider rules that depend on exchange.
        let mut by_group: std::collections::HashMap<
            (borsa_core::AssetKind, Option<borsa_core::Exchange>),
            Vec<Instrument>,
        > = std::collections::HashMap::new();
        for inst in instruments.iter().cloned() {
            by_group
                .entry((*inst.kind(), inst.exchange().cloned()))
                .or_default()
                .push(inst);
        }

        let resolved_backoff: BackoffConfig =
            backoff_override.or(self.cfg.backoff).unwrap_or_default();

        // For each kind, spin up a supervisor loop identical to previous logic, then fan-in.
        let (tx, rx) = tokio::sync::mpsc::channel::<borsa_core::QuoteUpdate>(1024);
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
        let (stop_broadcast_tx, stop_broadcast_rx) = tokio::sync::watch::channel(false);

        let mut joins = Vec::new();
        let mut init_receivers: Vec<tokio::sync::oneshot::Receiver<Result<(), BorsaError>>> =
            Vec::new();
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
                let candidates: Vec<&std::sync::Arc<dyn BorsaConnector>> = self
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

            // Build per-provider instrument subsets and allow-sets
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

            let (init_tx, init_rx) = tokio::sync::oneshot::channel();

            let params = KindSupervisorParams {
                providers,
                provider_instruments,
                provider_allow,
                min_backoff_ms,
                max_backoff_ms,
                factor,
                jitter_percent: u32::from(jitter_percent),
                initial_notify: Some(init_tx),
            };
            let join = Self::spawn_kind_supervisor(params, stop_broadcast_rx.clone(), tx.clone());
            joins.push(join);
            init_receivers.push(init_rx);
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

        // Supervisor to await stop and then abort all children
        let supervisor = tokio::spawn(async move {
            let _ = stop_rx.await;
            let _ = stop_broadcast_tx.send(true);
            for j in joins {
                let _ = j.await;
            }
        });

        Ok((
            borsa_core::stream::StreamHandle::new(supervisor, stop_tx),
            rx,
        ))
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
    ) -> Result<
        (
            borsa_core::stream::StreamHandle,
            tokio::sync::mpsc::Receiver<borsa_core::QuoteUpdate>,
        ),
        borsa_core::BorsaError,
    > {
        self.stream_quotes_with_backoff(instruments, None).await
    }
}

fn jitter_wait(base_ms: u64, jitter_percent: u32) -> u64 {
    let jitter_range = if jitter_percent == 0 {
        1
    } else {
        std::cmp::max(1, (base_ms.saturating_mul(u64::from(jitter_percent))) / 100)
    };
    let mut rng = rand::rng();
    base_ms + rng.random_range(0..jitter_range)
}

fn collapse_stream_errors(errors: Vec<BorsaError>) -> BorsaError {
    let mut actionable: Vec<BorsaError> = errors
        .into_iter()
        .flat_map(borsa_core::BorsaError::flatten)
        .filter(borsa_core::BorsaError::is_actionable)
        .collect();
    match actionable.len() {
        0 => BorsaError::unsupported(borsa_core::Capability::StreamQuotes.to_string()),
        1 => actionable.remove(0),
        _ => BorsaError::AllProvidersFailed(actionable),
    }
}

struct KindSupervisorParams {
    providers: Vec<std::sync::Arc<dyn BorsaConnector>>,
    /// Assigned instruments per provider, aligned by index with `providers`.
    provider_instruments: Vec<Vec<Instrument>>,
    /// Allowed symbol set per provider, aligned by index with `providers`.
    provider_allow: Vec<HashSet<String>>,
    min_backoff_ms: u64,
    max_backoff_ms: u64,
    factor: u32,
    jitter_percent: u32,
    initial_notify: Option<tokio::sync::oneshot::Sender<Result<(), BorsaError>>>,
}

struct EligibleStreamProviders {
    /// Providers eligible for this (kind, exchange) group sorted by score and registration order
    providers: Vec<std::sync::Arc<dyn BorsaConnector>>,
    /// Allowed symbols per provider, aligned with `providers`
    provider_symbols: Vec<HashSet<String>>,
    /// Union of all allowed symbols across providers
    union_symbols: HashSet<String>,
}

impl Borsa {
    fn eligible_stream_providers_for_context(
        &self,
        kind: borsa_core::AssetKind,
        exchange: Option<&borsa_core::Exchange>,
        instruments: &[Instrument],
    ) -> Result<EligibleStreamProviders, borsa_core::BorsaError> {
        // Score all connectors by the minimum per-symbol rank across the requested instruments,
        // then sort by (min_rank, registration_index). Collect allowed symbols in the process.
        let mut scored: Vec<StreamProviderScore> = Vec::new();

        for (orig_idx, connector) in self.connectors.iter().cloned().enumerate() {
            if connector.as_stream_provider().is_none() {
                continue;
            }
            if !connector.supports_kind(kind) {
                continue;
            }

            let mut allowed_syms: HashSet<String> = HashSet::new();
            let mut min_rank: usize = usize::MAX;
            for inst in instruments {
                let ctx = RoutingContext::new(
                    Some(inst.symbol_str()),
                    Some(kind),
                    inst.exchange().cloned().or_else(|| exchange.cloned()),
                );
                if let Some((rank, _strict)) = self
                    .cfg
                    .routing_policy
                    .providers
                    .provider_rank(&ctx, &connector.key())
                {
                    allowed_syms.insert(inst.symbol().to_string());
                    if rank < min_rank {
                        min_rank = rank;
                    }
                }
            }

            if !allowed_syms.is_empty() {
                scored.push((min_rank, orig_idx, connector, allowed_syms));
            }
        }

        if scored.is_empty() {
            return Err(borsa_core::BorsaError::unsupported(
                borsa_core::Capability::StreamQuotes.to_string(),
            ));
        }

        scored.sort_by_key(|(min_rank, orig_idx, _, _)| (*min_rank, *orig_idx));

        let mut providers: Vec<std::sync::Arc<dyn BorsaConnector>> = Vec::new();
        let mut provider_symbols: Vec<HashSet<String>> = Vec::new();
        let mut union_symbols: HashSet<String> = HashSet::new();

        for (_, _, c, syms) in scored {
            union_symbols.extend(syms.iter().cloned());
            providers.push(c);
            provider_symbols.push(syms);
        }

        Ok(EligibleStreamProviders {
            providers,
            provider_symbols,
            union_symbols,
        })
    }

    #[allow(clippy::too_many_lines)]
    fn spawn_kind_supervisor(
        params: KindSupervisorParams,
        mut stop_watch: tokio::sync::watch::Receiver<bool>,
        tx_clone: tokio::sync::mpsc::Sender<borsa_core::QuoteUpdate>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            use tokio::time::{Duration, sleep};
            let KindSupervisorParams {
                providers,
                provider_instruments,
                provider_allow,
                min_backoff_ms,
                max_backoff_ms,
                factor,
                jitter_percent,
                mut initial_notify,
            } = params;
            let mut start_index: usize = 0;
            let mut backoff_ms: u64 = min_backoff_ms;
            let mut initial_errors: Vec<BorsaError> = Vec::new();
            loop {
                let mut connected = false;
                let mut i = start_index;
                while i < providers.len() {
                    let Some(sp) = providers[i].as_stream_provider() else {
                        i += 1;
                        continue;
                    };
                    // Skip providers with no assigned instruments.
                    if provider_instruments
                        .get(i)
                        .is_none_or(std::vec::Vec::is_empty)
                    {
                        i += 1;
                        continue;
                    }
                    match sp.stream_quotes(&provider_instruments[i]).await {
                        Ok((handle, mut prx)) => {
                            connected = true;
                            if let Some(tx) = initial_notify.take() {
                                let _ = tx.send(Ok(()));
                            }
                            initial_errors.clear();

                            let mut provider_handle = Some(handle);
                            let allowed = provider_allow.get(i);
                            loop {
                                tokio::select! {
                                    biased;
                                    _ = stop_watch.changed() => {
                                        if *stop_watch.borrow() {
                                            if let Some(h) = provider_handle.take() { h.stop().await; }
                                            return;
                                        }
                                    }
                                    () = async {}, if *stop_watch.borrow() => {
                                        if let Some(h) = provider_handle.take() { h.stop().await; }
                                        return;
                                    }
                                    maybe_u = prx.recv() => {
                                        if let Some(u) = maybe_u {
                                            let mut pass = true;
                                            if let Some(allowset) = allowed
                                                && !allowset.contains(u.symbol.as_str()) {
                                                    pass = false;
                                                    #[cfg(feature = "tracing")]
                                                    tracing::warn!(symbol = %u.symbol, provider_index = i, "dropping update for unassigned symbol");
                                                }
                                            if pass && tx_clone.send(u).await.is_err() {
                                                    if let Some(h) = provider_handle.take() { h.abort(); }
                                                    return;
                                                }
                                        } else {
                                            if let Some(h) = provider_handle.take() { h.abort(); }
                                            break;
                                        }
                                    }
                                }
                            }

                            start_index = (i + 1) % providers.len();
                            break;
                        }
                        Err(err) => {
                            if initial_notify.is_some() {
                                initial_errors.push(crate::core::tag_err(providers[i].name(), err));
                            }
                            i += 1;
                        }
                    }
                }

                // Apply backoff after each cycle to avoid rapid reconnect loops.
                let base_ms = backoff_ms;
                let wait_ms = jitter_wait(base_ms, jitter_percent);

                tokio::select! {
                    _ = stop_watch.changed() => { if *stop_watch.borrow() { return; } }
                    () = sleep(Duration::from_millis(wait_ms)) => {}
                }

                if connected {
                    // Successful session: reset backoff to minimum before retrying.
                    backoff_ms = min_backoff_ms;
                } else {
                    if let Some(tx) = initial_notify.take() {
                        let err = collapse_stream_errors(std::mem::take(&mut initial_errors));
                        let _ = tx.send(Err(err));
                        return;
                    }
                    // No provider connected: increase backoff and restart from the first provider.
                    backoff_ms =
                        std::cmp::min(max_backoff_ms, base_ms.saturating_mul(u64::from(factor)));
                    start_index = 0;
                }
            }
        })
    }
}
