use crate::{BackoffConfig, Borsa};
use borsa_core::{BorsaConnector, BorsaError, Instrument};
use rand::Rng;

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
    pub async fn stream_quotes_with_backoff(
        &self,
        instruments: &[Instrument],
        backoff_override: Option<BackoffConfig>,
    ) -> Result<
        (
            borsa_core::StreamHandle,
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

        // Group instruments by kind to respect provider supports_kind checks and priorities.
        let mut by_kind: std::collections::HashMap<borsa_core::AssetKind, Vec<Instrument>> =
            std::collections::HashMap::new();
        for inst in instruments.iter().cloned() {
            by_kind.entry(*inst.kind()).or_default().push(inst);
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
        for (kind, list) in by_kind {
            let providers = self.eligible_stream_providers(kind)?;
            let allow: std::collections::HashSet<String> =
                list.iter().map(|i| i.symbol().to_string()).collect();
            let instruments_vec = list.clone();

            let min_backoff_ms = resolved_backoff.min_backoff_ms;
            let max_backoff_ms = resolved_backoff.max_backoff_ms;
            let factor = resolved_backoff.factor.max(1);
            let jitter_percent = resolved_backoff.jitter_percent.min(100);

            let (init_tx, init_rx) = tokio::sync::oneshot::channel();

            let params = KindSupervisorParams {
                providers,
                instruments: instruments_vec,
                allow,
                min_backoff_ms,
                max_backoff_ms,
                factor,
                jitter_percent: jitter_percent.into(),
                initial_notify: Some(init_tx),
            };
            let join = Self::spawn_kind_supervisor(params, stop_broadcast_rx.clone(), tx.clone());
            joins.push(join);
            init_receivers.push(init_rx);
        }

        // Ensure at least one kind connected successfully before returning a handle.
        let mut init_errors: Vec<BorsaError> = Vec::new();
        let mut any_success = false;
        for rx in init_receivers {
            match rx.await {
                Ok(Ok(())) => {
                    any_success = true;
                }
                Ok(Err(e)) => init_errors.push(e),
                Err(_) => init_errors.push(BorsaError::Other(
                    "stream supervisor dropped before initialization".into(),
                )),
            }
        }

        if !any_success {
            for join in &joins {
                join.abort();
            }
            let _ = stop_broadcast_tx.send(true);
            return Err(collapse_stream_errors(init_errors));
        }

        // Supervisor to await stop and then abort all children
        let supervisor = tokio::spawn(async move {
            let _ = stop_rx.await;
            let _ = stop_broadcast_tx.send(true);
            for j in joins {
                let _ = j.await;
            }
        });

        Ok((borsa_core::StreamHandle::new(supervisor, stop_tx), rx))
    }

    /// Start streaming quotes using the configured backoff settings.
    ///
    /// Notes:
    /// - Convenience wrapper around [`stream_quotes_with_backoff`] using the builder
    ///   configuration (or defaults) for backoff.
    /// # Errors
    /// Returns an error if streaming initialization fails for all providers.
    pub async fn stream_quotes(
        &self,
        instruments: &[Instrument],
    ) -> Result<
        (
            borsa_core::StreamHandle,
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

fn collapse_stream_errors(mut errors: Vec<BorsaError>) -> BorsaError {
    match errors.len() {
        0 => BorsaError::unsupported("stream-quotes"),
        1 => errors.remove(0),
        _ => BorsaError::AllProvidersFailed(errors),
    }
}

struct KindSupervisorParams {
    providers: Vec<std::sync::Arc<dyn BorsaConnector>>,
    instruments: Vec<Instrument>,
    allow: std::collections::HashSet<String>,
    min_backoff_ms: u64,
    max_backoff_ms: u64,
    factor: u32,
    jitter_percent: u32,
    initial_notify: Option<tokio::sync::oneshot::Sender<Result<(), BorsaError>>>,
}

impl Borsa {
    fn eligible_stream_providers(
        &self,
        kind: borsa_core::AssetKind,
    ) -> Result<Vec<std::sync::Arc<dyn BorsaConnector>>, borsa_core::BorsaError> {
        let ordered = self.ordered_for_kind(Some(kind));
        let mut eligible: Vec<std::sync::Arc<dyn BorsaConnector>> = ordered
            .into_iter()
            .filter(|c| c.as_stream_provider().is_some())
            .collect();
        eligible.retain(|c| c.supports_kind(kind));
        if eligible.is_empty() {
            return Err(borsa_core::BorsaError::unsupported("stream-quotes"));
        }
        Ok(eligible)
    }

    fn spawn_kind_supervisor(
        params: KindSupervisorParams,
        mut stop_watch: tokio::sync::watch::Receiver<bool>,
        tx_clone: tokio::sync::mpsc::Sender<borsa_core::QuoteUpdate>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            use tokio::time::{Duration, sleep};
            let KindSupervisorParams {
                providers,
                instruments,
                allow,
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
                    match sp.stream_quotes(&instruments).await {
                        Ok((handle, mut prx)) => {
                            connected = true;
                            if let Some(tx) = initial_notify.take() {
                                let _ = tx.send(Ok(()));
                            }
                            initial_errors.clear();

                            let mut provider_handle = Some(handle);
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
                                            if allow.contains(u.symbol.as_str()) &&
                                                tx_clone.send(u).await.is_err()
                                            {
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
