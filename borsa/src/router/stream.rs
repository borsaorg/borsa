use crate::router::streaming::{
    EligibleStreamProviders, KindSupervisorParams, StreamUpdateKind, collapse_stream_errors,
    spawn_kind_supervisor,
};
use crate::{BackoffConfig, Borsa};
use borsa_core::{
    AssetKind, BorsaConnector, BorsaError, CandleUpdate, Capability, Exchange, Instrument,
    Interval, OptionUpdate, QuoteUpdate, RoutingContext, Symbol, stream::StreamHandle,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, watch};

impl Borsa {
    #[allow(clippy::too_many_lines)]
    async fn stream_updates_with_backoff<T, F>(
        &self,
        instruments: &[Instrument],
        context: T::Context,
        backoff_override: Option<BackoffConfig>,
        capability: Capability,
        eligible_fn: F,
    ) -> Result<(StreamHandle, mpsc::Receiver<T>), BorsaError>
    where
        T: StreamUpdateKind,
        T::Context: Clone,
        F: Fn(
            &Self,
            AssetKind,
            Option<&Exchange>,
            &[Instrument],
        ) -> Result<EligibleStreamProviders, BorsaError>,
    {
        tokio::task::yield_now().await;
        if instruments.is_empty() {
            return Err(borsa_core::BorsaError::InvalidArg(
                "instruments list cannot be empty".into(),
            ));
        }

        let mut by_group: HashMap<(AssetKind, Option<Exchange>), Vec<Instrument>> = HashMap::new();
        for inst in instruments.iter().cloned() {
            let exch_opt = match inst.id() {
                borsa_core::IdentifierScheme::Security(sec) => sec.exchange.clone(),
                borsa_core::IdentifierScheme::Prediction(_) => None,
            };
            by_group
                .entry((*inst.kind(), exch_opt))
                .or_default()
                .push(inst);
        }

        let resolved_backoff: BackoffConfig =
            backoff_override.or(self.cfg.backoff).unwrap_or_default();

        let (tx, rx) = mpsc::channel::<T>(1024);
        let (stop_tx, stop_rx) = oneshot::channel::<()>();
        let (stop_broadcast_tx, stop_broadcast_rx) = watch::channel(false);

        let mut joins = Vec::new();
        let mut init_receivers: Vec<oneshot::Receiver<Result<(), BorsaError>>> = Vec::new();
        for ((kind, ex), list) in by_group {
            let EligibleStreamProviders {
                providers,
                provider_symbols,
                union_symbols,
            } = eligible_fn(self, kind, ex.as_ref(), &list)?;
            if union_symbols.is_empty() {
                continue;
            }

            let mut list_pairs: Vec<(Instrument, Symbol)> = Vec::with_capacity(list.len());
            for inst in list {
                let sym = match inst.id() {
                    borsa_core::IdentifierScheme::Security(sec) => sec.symbol.clone(),
                    borsa_core::IdentifierScheme::Prediction(_) => {
                        return Err(BorsaError::unsupported(
                            "instrument scheme (stream/security-only)",
                        ));
                    }
                };
                list_pairs.push((inst, sym));
            }

            let requested: HashSet<Symbol> = list_pairs.iter().map(|(_, s)| s.clone()).collect();
            let rejected: Vec<Symbol> = requested.difference(&union_symbols).cloned().collect();
            if !rejected.is_empty() {
                let mut strict_filtered: Vec<Symbol> = Vec::new();
                let candidates: Vec<&Arc<dyn BorsaConnector>> = self
                    .connectors
                    .iter()
                    .filter(|c| match capability {
                        Capability::StreamQuotes => c.as_stream_provider().is_some(),
                        Capability::StreamOptions => c.as_option_stream_provider().is_some(),
                        Capability::StreamCandles => c.as_candle_stream_provider().is_some(),
                        _ => false,
                    } && c.supports_kind(kind))
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

            let mut group_has_explicit: bool = false;
            'outer: for (_, sym) in &list_pairs {
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
                let mut primary_groups: HashMap<usize, Vec<Symbol>> = HashMap::new();
                for sym in &requested {
                    if !union_symbols.contains(sym) {
                        continue;
                    }
                    let mut ranked: Vec<(usize, usize)> = Vec::new();
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
                            ranked.push((rank, idx));
                        }
                    }
                    if ranked.is_empty() {
                        continue;
                    }
                    ranked.sort_by_key(|(rank, providers_idx)| (*rank, *providers_idx));
                    let (_best_rank, best_idx) = ranked[0];
                    primary_groups
                        .entry(best_idx)
                        .or_default()
                        .push(sym.clone());
                }

                for (primary_idx, group_syms) in primary_groups {
                    let group_syms_set: HashSet<Symbol> = group_syms.iter().cloned().collect();

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
                        let assigned = list_pairs
                            .iter()
                            .filter(|(_, sym)| filtered_allow.contains(sym))
                            .map(|(inst, _)| inst.clone())
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
                    let context_arc = Arc::new(context.clone());
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
                        capability,
                        context: context_arc,
                    };
                    let join =
                        spawn_kind_supervisor::<T>(params, stop_broadcast_rx.clone(), tx.clone());
                    joins.push(join);
                    init_receivers.push(init_rx);
                }
            } else {
                let mut provider_instruments: Vec<Vec<Instrument>> =
                    Vec::with_capacity(providers.len());
                let mut provider_allow: Vec<HashSet<Symbol>> = Vec::with_capacity(providers.len());
                for allow in &provider_symbols {
                    let assigned = list_pairs
                        .iter()
                        .filter(|(_, sym)| allow.contains(sym))
                        .map(|(inst, _)| inst.clone())
                        .collect::<Vec<_>>();
                    provider_instruments.push(assigned);
                    provider_allow.push(allow.clone());
                }

                let min_backoff_ms = resolved_backoff.min_backoff_ms;
                let max_backoff_ms = resolved_backoff.max_backoff_ms;
                let factor = resolved_backoff.factor.max(1);
                let jitter_percent = resolved_backoff.jitter_percent.min(100);

                let (init_tx, init_rx) = oneshot::channel();

                let required_symbols: HashSet<Symbol> = list_pairs
                    .iter()
                    .filter(|(_, sym)| union_symbols.contains(sym))
                    .map(|(_, sym)| sym.clone())
                    .collect();

                let context_arc = Arc::new(context.clone());
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
                    capability,
                    context: context_arc,
                };
                let join =
                    spawn_kind_supervisor::<T>(params, stop_broadcast_rx.clone(), tx.clone());
                joins.push(join);
                init_receivers.push(init_rx);
            }
        }

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
            let err = collapse_stream_errors(capability, init_errors);
            for join in joins {
                join.abort();
            }
            let _ = stop_broadcast_tx.send(true);
            return Err(err);
        }

        let supervisor = tokio::spawn(async move {
            let mut stop_rx_inner = stop_rx;
            tokio::select! {
                _ = &mut stop_rx_inner => {}
                () = tx.closed() => {}
            }
            let _ = stop_broadcast_tx.send(true);
            for j in joins {
                let _ = j.await;
            }
        });

        Ok((StreamHandle::new(supervisor, stop_tx), rx))
    }

    /// Start streaming quotes with automatic backoff, provider failover, and policy-aware routing.
    ///
    /// Parameters:
    /// - `instruments`: non-empty list of instruments to stream
    /// - `backoff_override`: optional backoff settings; defaults to config/built-ins
    ///
    /// Grouping:
    /// - Instruments are grouped by `(AssetKind, Option<Exchange>)` to respect exchange-sensitive
    ///   routing rules. Each group is managed independently and updates are fanned-in to a single
    ///   channel.
    ///
    /// Provider eligibility and ordering:
    /// - For each group, eligible streaming providers are scored by the minimum per-symbol rank
    ///   across the requested instruments (from the routing policy), with ties broken by
    ///   registration order. Allowed symbol sets are tracked per provider.
    ///
    /// Routing modes:
    /// - Per-symbol preferences: if any instrument in a group has an explicit provider preference
    ///   (rank != `usize::MAX`), a primary provider is resolved per symbol from the eligible set
    ///   using the lowest rank (then stable tie-break). One supervisor is spawned per primary
    ///   provider, with a fallback chain (primary first, then others). Each supervisor subscribes
    ///   only to the symbols assigned to its primary; overlapping lower-priority sessions are
    ///   preempted when a higher-priority session activates.
    /// - Group-level fallback: if no explicit per-symbol preferences exist, a single supervisor
    ///   manages the whole group, attempting providers in scored order and subscribing each only
    ///   to the symbols it is allowed to stream. Required symbols are the union of allowed symbols
    ///   across providers for the requested set.
    ///
    /// Strict policy handling:
    /// - If strict routing rules exclude requested symbols despite available streaming providers,
    ///   an error is returned listing the rejected symbols.
    ///
    /// Backoff and termination:
    /// - Exponential backoff with jitter is used between attempts; successful activation resets
    ///   backoff.
    /// - Optional monotonic timestamp enforcement is applied if enabled in config.
    /// - Dropping or stopping the returned `StreamHandle` terminates all supervised tasks.
    ///
    /// # Errors
    /// - Returns an error if initialization fails across all providers for all groups, or when no
    ///   streaming-capable providers are available.
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
        self.stream_updates_with_backoff::<QuoteUpdate, _>(
            instruments,
            (),
            backoff_override,
            Capability::StreamQuotes,
            |this, kind, exchange, list| {
                this.eligible_stream_providers_for_context(kind, exchange, list)
            },
        )
        .await
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

    /// Start streaming candle updates with automatic backoff, provider failover, and policy-aware routing.
    ///
    /// Parameters mirror [`Self::stream_quotes_with_backoff`] with an additional `interval`
    /// argument that selects the provider-native candle cadence.
    ///
    /// The routing, strict policy handling, and supervisor behavior match the quote-streaming
    /// implementation; the only difference is that providers must advertise
    /// [`CandleStreamProvider`](borsa_core::connector::CandleStreamProvider) and will produce
    /// [`CandleUpdate`] frames with `is_final` flagged when upstream closes the interval.
    ///
    /// # Errors
    /// Returns an error if candle-capable providers cannot be started for any requested group or
    /// when strict routing rules reject every symbol.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa::router::stream_candles_with_backoff",
            skip(self, instruments, interval, backoff_override)
        )
    )]
    #[allow(clippy::too_many_lines)]
    pub async fn stream_candles_with_backoff(
        &self,
        instruments: &[Instrument],
        interval: Interval,
        backoff_override: Option<BackoffConfig>,
    ) -> Result<(StreamHandle, mpsc::Receiver<CandleUpdate>), BorsaError> {
        self.stream_updates_with_backoff::<CandleUpdate, _>(
            instruments,
            interval,
            backoff_override,
            Capability::StreamCandles,
            |this, kind, exchange, list| {
                this.eligible_candle_stream_providers_for_context(kind, exchange, list)
            },
        )
        .await
    }

    /// Start streaming candles using configured backoff settings.
    ///
    /// # Errors
    /// Propagates the same conditions as [`Self::stream_candles_with_backoff`].
    pub async fn stream_candles(
        &self,
        instruments: &[Instrument],
        interval: Interval,
    ) -> Result<(StreamHandle, mpsc::Receiver<CandleUpdate>), BorsaError> {
        self.stream_candles_with_backoff(instruments, interval, None)
            .await
    }

    /// Start streaming option updates with automatic backoff, provider failover, and policy-aware routing.
    ///
    /// Parameters:
    /// - `instruments`: non-empty list of instruments to stream
    /// - `backoff_override`: optional backoff settings; defaults to config/built-ins
    ///
    /// Grouping and routing behavior mirrors `stream_quotes_with_backoff`.
    ///
    /// Backoff and termination:
    /// - Exponential backoff with jitter is used between attempts; successful activation resets
    ///   backoff.
    /// - Optional monotonic timestamp enforcement is applied if enabled in config.
    /// - Dropping or stopping the returned `StreamHandle` terminates all supervised tasks.
    ///
    /// # Errors
    /// - Returns an error if initialization fails across all providers for all groups, or when no
    ///   streaming-capable providers are available.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa::router::stream_options_with_backoff",
            skip(self, instruments, backoff_override)
        )
    )]
    #[allow(clippy::too_many_lines)]
    pub async fn stream_options_with_backoff(
        &self,
        instruments: &[Instrument],
        backoff_override: Option<BackoffConfig>,
    ) -> Result<(StreamHandle, mpsc::Receiver<OptionUpdate>), BorsaError> {
        self.stream_updates_with_backoff::<OptionUpdate, _>(
            instruments,
            (),
            backoff_override,
            Capability::StreamOptions,
            |this, kind, exchange, list| {
                this.eligible_option_stream_providers_for_context(kind, exchange, list)
            },
        )
        .await
    }

    /// Start streaming options using the configured backoff settings.
    ///
    /// # Errors
    /// Returns an error if streaming initialization fails for all providers.
    pub async fn stream_options(
        &self,
        instruments: &[Instrument],
    ) -> Result<(StreamHandle, mpsc::Receiver<OptionUpdate>), BorsaError> {
        self.stream_options_with_backoff(instruments, None).await
    }
}
