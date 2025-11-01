use crate::router::streaming::{
    EligibleStreamProviders, KindSupervisorParams, collapse_stream_errors, spawn_kind_supervisor,
};
use crate::{BackoffConfig, Borsa};
use borsa_core::{
    AssetKind, BorsaConnector, BorsaError, Exchange, Instrument, QuoteUpdate, RoutingContext,
    Symbol, stream::StreamHandle,
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
            let requested: HashSet<Symbol> = list.iter().map(|i| i.symbol().clone()).collect();
            let rejected: Vec<Symbol> = requested.difference(&union_symbols).cloned().collect();
            if !rejected.is_empty() {
                // Determine if strict rules excluded these symbols (vs capability absence).
                let mut strict_filtered: Vec<Symbol> = Vec::new();
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
                            let ctx = RoutingContext::new(Some(sym), Some(kind), ex.clone());
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
                let sym = inst.symbol();
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
                // When any explicit preference exists in this group, resolve a primary provider
                // for every eligible symbol (explicit or wildcard) and group by that provider.
                let mut primary_groups: HashMap<usize, Vec<Symbol>> = HashMap::new();
                for sym in &requested {
                    if !union_symbols.contains(sym) {
                        continue;
                    }
                    // Gather candidate providers that allow this symbol
                    let mut candidates: Vec<(usize, usize)> = Vec::new(); // (rank, providers_idx)
                    for (idx, p) in providers.iter().enumerate() {
                        if !provider_symbols
                            .get(idx)
                            .is_some_and(|set| set.contains(sym))
                        {
                            continue;
                        }
                        let ctx = RoutingContext::new(Some(sym), Some(kind), ex.clone());
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
                    let group_syms_set: HashSet<Symbol> = group_syms.iter().cloned().collect();

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
                    let mut provider_allow: Vec<HashSet<Symbol>> =
                        Vec::with_capacity(chain_indices.len());

                    for &orig_idx in &chain_indices {
                        chain_providers.push(providers[orig_idx].clone());
                        let allow_full =
                            provider_symbols.get(orig_idx).cloned().unwrap_or_default();
                        let filtered_allow: HashSet<Symbol> = allow_full
                            .into_iter()
                            .filter(|s| group_syms_set.contains(s))
                            .collect();
                        let assigned = list
                            .iter()
                            .filter(|&inst| filtered_allow.contains(inst.symbol()))
                            .cloned()
                            .collect::<Vec<_>>();
                        provider_instruments.push(assigned);
                        provider_allow.push(filtered_allow);
                    }

                    let min_backoff_ms = resolved_backoff.min_backoff_ms;
                    let max_backoff_ms = resolved_backoff.max_backoff_ms;
                    let factor = resolved_backoff.factor.max(1);
                    let jitter_percent = resolved_backoff.jitter_percent.min(100);

                    let required_symbols: HashSet<Symbol> = group_syms_set.clone();
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

                // No separate wildcard-only supervisor; wildcard symbols were merged into
                // their resolved primary provider groups above.
            } else {
                // No explicit preferences for this group: use group-level supervisor (fallback allowed)
                let mut provider_instruments: Vec<Vec<Instrument>> =
                    Vec::with_capacity(providers.len());
                let mut provider_allow: Vec<HashSet<Symbol>> = Vec::with_capacity(providers.len());
                for allow in &provider_symbols {
                    let assigned = list
                        .iter()
                        .filter(|&inst| allow.contains(inst.symbol()))
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

                let required_symbols: HashSet<Symbol> = list
                    .iter()
                    .filter(|inst| union_symbols.contains(inst.symbol()))
                    .map(|inst| inst.symbol().clone())
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
                () = tx.closed() => {}
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
