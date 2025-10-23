#[cfg(feature = "tracing")]
use std::convert::TryFrom;
use std::sync::Arc;

use borsa_core::types::{BackoffConfig, BorsaConfig, FetchStrategy, MergeStrategy, Resampling};
use borsa_core::{AssetKind, BorsaConnector, BorsaError, Instrument, RoutingContext};

/// Orchestrator that routes requests across registered providers.
pub struct Borsa {
    pub(crate) connectors: Vec<Arc<dyn BorsaConnector>>,
    pub(crate) cfg: BorsaConfig,
}

/// Builder for constructing a `Borsa` orchestrator with custom configuration.
pub struct BorsaBuilder {
    connectors: Vec<Arc<dyn BorsaConnector>>,
    cfg: BorsaConfig,
}

impl Default for BorsaBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl BorsaBuilder {
    /// Create a new builder with sensible defaults.
    ///
    /// Behavior and trade-offs:
    /// - Starts with no connectors; you must register at least one via `with_connector`.
    /// - Defaults are conservative: no resampling, no adjusted-history preference,
    ///   priority-with-fallback fetches, deep merge for history, 5s provider timeout.
    /// - Use the builder modifiers below to steer provider selection, merging,
    ///   resampling, and streaming backoff to fit your use case.
    #[must_use]
    pub fn new() -> Self {
        Self {
            connectors: vec![],
            cfg: BorsaConfig::default(),
        }
    }

    /// Register a provider connector.
    ///
    /// Behavior and trade-offs:
    /// - The order in which you register connectors is used only when no explicit
    ///   priorities are set via a custom [`RoutingPolicy`](borsa_core::RoutingPolicy).
    /// - Multiple connectors can support the same capability; the orchestrator will
    ///   route based on priorities and the selected fetch/merge strategies.
    /// - Duplicates are not deduplicated; avoid registering the same connector twice.
    #[must_use]
    pub fn with_connector(mut self, c: Arc<dyn BorsaConnector>) -> Self {
        self.connectors.push(c);
        self
    }

    /// Set the unified routing policy controlling provider and exchange ordering.
    ///
    /// Semantics:
    /// - Provider eligibility and ordering are driven by provider rules in the
    ///   policy. Unknown connector keys are rejected at build time.
    /// - Exchange preferences influence search result de-duplication only; they
    ///   do not change provider eligibility.
    #[must_use]
    pub fn routing_policy(mut self, policy: borsa_core::RoutingPolicy) -> Self {
        self.cfg.routing_policy = policy;
        self
    }

    /// Toggle preference for adjusted history when merging.
    ///
    /// Behavior and trade-offs:
    /// - When enabled, adjusted series are preferred in the merge ordering which can
    ///   reduce discontinuities around corporate actions at the cost of differing from
    ///   unadjusted close values.
    /// - If resampling is performed later, per-candle `close_unadj` will be cleared to avoid
    ///   ambiguity across providers and cadences.
    #[must_use]
    pub const fn prefer_adjusted_history(mut self, yes: bool) -> Self {
        self.cfg.prefer_adjusted_history = yes;
        self
    }

    /// Select forced resampling mode for merged history.
    ///
    /// Behavior and trade-offs:
    /// - `Daily` and `Weekly` normalize cadence after merging. This simplifies
    ///   downstream analytics but discards native provider periodicity.
    /// - Any forced resample clears provider-specific per-candle `close_unadj` to prevent
    ///   mixing raw values from different cadences/providers.
    #[must_use]
    pub const fn resampling(mut self, mode: Resampling) -> Self {
        self.cfg.resampling = mode;
        self
    }

    /// Automatically resample subdaily series to daily when appropriate.
    ///
    /// Behavior and trade-offs:
    /// - If a request results in a subdaily effective cadence, the merged series is
    ///   upsampled to daily. This stabilizes time alignment across sources but loses
    ///   intraday detail.
    /// - Like forced resampling, this clears per-candle `close_unadj`.
    #[must_use]
    pub const fn auto_resample_subdaily_to_daily(mut self, yes: bool) -> Self {
        self.cfg.auto_resample_subdaily_to_daily = yes;
        self
    }

    /// Select the fetch strategy for multi-provider requests.
    ///
    /// Behavior and trade-offs:
    /// - `PriorityWithFallback`: deterministic order, applies per-provider timeout,
    ///   aggregates errors; may be slower but predictable and economical on rate limits.
    /// - `Latency`: race all eligible providers and return the first success; fastest
    ///   typical latency but consumes more concurrent requests and can add load.
    #[must_use]
    pub const fn fetch_strategy(mut self, strategy: FetchStrategy) -> Self {
        self.cfg.fetch_strategy = strategy;
        self
    }

    /// Select the merge strategy for history data from multiple providers.
    ///
    /// Behavior and trade-offs:
    /// - `Deep`: fetch all eligible providers concurrently and merge to backfill gaps;
    ///   produces the most complete series at the cost of more requests.
    /// - `Fallback`: iterate providers until the first non-empty dataset; minimizes API
    ///   usage but may miss data available from lower-priority providers.
    #[must_use]
    pub const fn merge_history_strategy(mut self, strategy: MergeStrategy) -> Self {
        self.cfg.merge_history_strategy = strategy;
        self
    }

    /// Set the per-provider request timeout.
    ///
    /// Behavior and trade-offs:
    /// - Applied in both `PriorityWithFallback` and `Latency` strategies to bound
    ///   each provider call.
    /// - In `Latency` mode, the first success wins while timeouts cap stragglers.
    #[must_use]
    pub const fn provider_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.cfg.provider_timeout = timeout;
        self
    }

    /// Set an overall request timeout for fan-out aggregations (history/search).
    ///
    /// Behavior and trade-offs:
    /// - Bounds total latency even when many providers time out sequentially.
    /// - When exceeded, returns a `RequestTimeout` error for the capability.
    #[must_use]
    pub const fn request_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.cfg.request_timeout = Some(timeout);
        self
    }

    /// Provide a custom backoff configuration for streaming.
    ///
    /// Behavior and trade-offs:
    /// - Controls reconnect delays and jitter in streaming failover.
    /// - Higher jitter reduces thundering herds but adds variance to reconnect times.
    #[must_use]
    pub const fn backoff(mut self, cfg: BackoffConfig) -> Self {
        self.cfg.backoff = Some(cfg);
        self
    }

    /// Build the `Borsa` orchestrator.
    ///
    /// # Errors
    /// - `InvalidArg` if no connectors have been registered via `with_connector`.
    /// - `InvalidArg` if the routing policy references unknown connector keys.
    pub fn build(mut self) -> Result<Borsa, BorsaError> {
        // Collect registered connector names for validation.
        let known: std::collections::HashSet<&'static str> =
            self.connectors.iter().map(|c| c.name()).collect();

        // Normalize provider rules and collect unknown connector references.
        let mut providers = std::mem::take(&mut self.cfg.routing_policy.providers);
        let unknown = providers.normalize_and_collect_unknown(&known);
        self.cfg.routing_policy.providers = providers;

        if !unknown.is_empty() {
            let details = unknown
                .into_iter()
                .map(|(selector, names)| format!("{selector:?}: {}", names.join(", ")))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(BorsaError::InvalidArg(format!(
                "routing policy references unknown connectors: {details}"
            )));
        }

        if self.connectors.is_empty() {
            return Err(BorsaError::InvalidArg(
                "no connectors registered; add at least one via with_connector(...)".to_string(),
            ));
        }

        Ok(Borsa {
            connectors: self.connectors,
            cfg: self.cfg,
        })
    }
}

pub fn tag_err(connector: &str, e: BorsaError) -> BorsaError {
    match e {
        e @ (BorsaError::NotFound { .. }
        | BorsaError::ProviderTimeout { .. }
        | BorsaError::Connector { .. }
        | BorsaError::RequestTimeout { .. }
        | BorsaError::AllProvidersTimedOut { .. }
        | BorsaError::AllProvidersFailed(_)) => e,
        other => BorsaError::Connector {
            connector: connector.to_string(),
            msg: other.to_string(),
        },
    }
}

impl Borsa {
    /// Enforce that a quote's exchange matches the instrument's desired exchange when provided.
    ///
    /// When the instrument specifies an exchange, a mismatched quote exchange is treated as
    /// `NotFound` to trigger fallback or allow latency racing to continue. Quotes missing an
    /// exchange pass through unchanged to preserve legacy behaviour.
    pub(crate) fn enforce_quote_exchange(
        inst: &Instrument,
        q: &borsa_core::Quote,
    ) -> Result<(), BorsaError> {
        let Some(want) = inst.exchange() else {
            return Ok(());
        };

        match q.exchange.as_ref() {
            Some(have) if have == want => Ok(()),
            Some(_) => Err(BorsaError::not_found(format!(
                "quote for {} (exchange mismatch)",
                inst.symbol()
            ))),
            None => Ok(()),
        }
    }

    pub(crate) fn dedup_search_results_by_exchange(
        &self,
        kind: Option<AssetKind>,
        merged: Vec<borsa_core::SearchResult>,
    ) -> Vec<borsa_core::SearchResult> {
        use std::collections::HashMap;
        let mut grouped: HashMap<String, Vec<(usize, borsa_core::SearchResult)>> = HashMap::new();
        for (i, r) in merged.into_iter().enumerate() {
            grouped
                .entry(r.symbol.as_str().to_string())
                .or_default()
                .push((i, r));
        }
        // Preserve overall provider order by selecting the best per symbol, then sorting by first-seen index.
        let mut selected: Vec<(usize, borsa_core::SearchResult)> =
            Vec::with_capacity(grouped.len());
        for (sym, mut group) in grouped {
            group.sort_by_key(|(i, r)| {
                let ctx_kind = kind.or(Some(r.kind));
                let ctx = RoutingContext::new(Some(&sym), ctx_kind, None);
                self.cfg
                    .routing_policy
                    .exchange_sort_key(&ctx, r.exchange.as_ref(), *i)
            });
            let first_index = group.iter().map(|(i, _)| *i).min().unwrap_or(usize::MAX);
            let best = group.remove(0).1;
            selected.push((first_index, best));
        }
        selected.sort_by_key(|(i, _)| *i);
        selected.into_iter().map(|(_, r)| r).collect()
    }
    /// Wrap a provider future with a timeout and standardized timeout error mapping.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa::core::provider_call_with_timeout",
            skip(fut),
            fields(
                connector = connector_name,
                capability = capability,
                timeout_ms = u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX),
            ),
        )
    )]
    pub(crate) async fn provider_call_with_timeout<T, Fut>(
        connector_name: &'static str,
        capability: &'static str,
        timeout: std::time::Duration,
        fut: Fut,
    ) -> Result<T, BorsaError>
    where
        Fut: core::future::Future<Output = Result<T, BorsaError>>,
    {
        (tokio::time::timeout(timeout, fut).await)
            .unwrap_or_else(|_| Err(BorsaError::provider_timeout(connector_name, capability)))
    }
    /// Start building a new `Borsa` instance.
    ///
    /// Typical usage chains provider registration and preferences, e.g.:
    ///
    /// ```rust,ignore
    /// use std::sync::Arc;
    /// use borsa_core::{AssetKind, connector::BorsaConnector};
    ///
    /// // Mock connector types for demonstration
    /// struct YfConnector;
    /// struct AvConnector;
    /// impl YfConnector { fn new_default() -> Self { Self } }
    /// impl AvConnector { fn new_with_key(_: &str) -> Self { Self } }
    ///
    /// // Mock implementations of BorsaConnector trait
    /// impl BorsaConnector for YfConnector {
    ///     fn name(&self) -> &'static str { "yf" }
    /// }
    /// impl BorsaConnector for AvConnector {
    ///     fn name(&self) -> &'static str { "av" }
    /// }
    ///
    /// let yf = Arc::new(YfConnector::new_default());
    /// let av = Arc::new(AvConnector::new_with_key("..."));
    ///
    /// let policy = borsa_core::RoutingPolicyBuilder::new()
    ///     .providers_for_kind(
    ///         AssetKind::Equity,
    ///         &[av.key(), yf.key()],
    ///     )
    ///     .build();
    ///
    /// let borsa = borsa::Borsa::builder()
    ///     .with_connector(yf.clone())
    ///     .with_connector(av.clone())
    ///     .routing_policy(policy)
    ///     .merge_history_strategy(borsa::MergeStrategy::Deep)
    ///     .fetch_strategy(borsa::FetchStrategy::PriorityWithFallback)
    ///     .build()?;
    /// ```
    #[must_use]
    pub fn builder() -> BorsaBuilder {
        BorsaBuilder::new()
    }

    pub(crate) fn ordered(&self, inst: &Instrument) -> Vec<Arc<dyn BorsaConnector>> {
        let mut out: Vec<(usize, Arc<dyn BorsaConnector>)> =
            self.connectors.iter().cloned().enumerate().collect();
        let ctx = RoutingContext::new(
            Some(inst.symbol_str()),
            Some(*inst.kind()),
            inst.exchange().cloned(),
        );
        out.retain(|(_, c)| {
            let key = c.key();
            self.cfg
                .routing_policy
                .providers
                .provider_rank(&ctx, &key)
                .is_some()
        });
        out.sort_by_key(|(orig_i, c)| {
            let key = c.key();
            self.cfg
                .routing_policy
                .provider_sort_key(&ctx, &key, *orig_i)
        });
        out.into_iter().map(|(_, c)| c).collect()
    }

    pub(crate) fn ordered_for_kind(&self, kind: Option<AssetKind>) -> Vec<Arc<dyn BorsaConnector>> {
        let mut out: Vec<(usize, Arc<dyn BorsaConnector>)> =
            self.connectors.iter().cloned().enumerate().collect();
        let ctx = RoutingContext::new(None, kind, None);
        out.retain(|(_, c)| {
            let key = c.key();
            self.cfg
                .routing_policy
                .providers
                .provider_rank(&ctx, &key)
                .is_some()
        });
        out.sort_by_key(|(orig_i, c)| {
            let key = c.key();
            self.cfg
                .routing_policy
                .provider_sort_key(&ctx, &key, *orig_i)
        });
        out.into_iter().map(|(_, c)| c).collect()
    }

    // removed: ordered_for_context (unused)

    // execute_fetch removed in favor of explicit provider routing per router

    /// Generic single-item fetch helper matching `quote` semantics.
    ///
    /// - Honors `FetchStrategy::{PriorityWithFallback, Latency}`
    /// - Applies per-provider timeout in both modes
    /// - Aggregates errors and treats `NotFound` specially in fallback mode
    /// - In latency mode, returns the first success; if all attempted providers fail,
    ///   aggregates and returns `AllProvidersFailed`; if no providers support the
    ///   capability, returns a capability error
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa::core::fetch_single",
            skip(self, call),
            fields(symbol = %inst.symbol(), capability = %capability_label, not_found = %not_found_label),
        )
    )]
    pub(crate) async fn fetch_single<T, F, Fut>(
        &self,
        inst: &Instrument,
        capability_label: &'static str,
        not_found_label: &'static str,
        call: F,
    ) -> Result<T, BorsaError>
    where
        T: Send,
        F: Fn(Arc<dyn BorsaConnector>, Instrument) -> Option<Fut> + Clone + Send,
        Fut: core::future::Future<Output = Result<T, BorsaError>> + Send,
    {
        match self.cfg.fetch_strategy {
            FetchStrategy::PriorityWithFallback => {
                self.fetch_single_priority_with_fallback(
                    inst,
                    capability_label,
                    not_found_label,
                    call,
                )
                .await
            }
            FetchStrategy::Latency => {
                self.fetch_single_latency(inst, capability_label, not_found_label, call)
                    .await
            }
            _ => Err(BorsaError::InvalidArg(
                "unknown fetch strategy (upgrade borsa to support this variant)".into(),
            )),
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa::core::fetch_single_priority_with_fallback",
            skip(self, call),
            fields(symbol = %inst.symbol(), capability = %capability_label, not_found = %not_found_label),
        )
    )]
    async fn fetch_single_priority_with_fallback<T, F, Fut>(
        &self,
        inst: &Instrument,
        capability_label: &'static str,
        not_found_label: &'static str,
        call: F,
    ) -> Result<T, BorsaError>
    where
        T: Send,
        F: Fn(Arc<dyn BorsaConnector>, Instrument) -> Option<Fut> + Clone + Send,
        Fut: core::future::Future<Output = Result<T, BorsaError>> + Send,
    {
        let mut attempted_any = false;
        let mut errors: Vec<BorsaError> = Vec::new();
        let mut all_not_found = true;

        for c in self.ordered(inst) {
            if let Some(fut) = call(c.clone(), inst.clone()) {
                attempted_any = true;
                match Self::provider_call_with_timeout(
                    c.name(),
                    capability_label,
                    self.cfg.provider_timeout,
                    fut,
                )
                .await
                {
                    Ok(v) => return Ok(v),
                    Err(e @ BorsaError::NotFound { .. }) => {
                        errors.push(e);
                    }
                    Err(e @ BorsaError::ProviderTimeout { .. }) => {
                        all_not_found = false;
                        errors.push(e);
                    }
                    Err(e) => {
                        all_not_found = false;
                        errors.push(crate::core::tag_err(c.name(), e));
                    }
                }
            }
        }

        if !attempted_any {
            return Err(BorsaError::unsupported(capability_label));
        }

        if all_not_found
            && !errors.is_empty()
            && errors
                .iter()
                .all(|e| matches!(e, BorsaError::NotFound { .. }))
        {
            return Err(BorsaError::not_found(format!(
                "{} for {}",
                not_found_label,
                inst.symbol()
            )));
        }

        if !errors.is_empty()
            && errors
                .iter()
                .all(|e| matches!(e, BorsaError::ProviderTimeout { .. }))
        {
            Err(BorsaError::AllProvidersTimedOut {
                capability: capability_label.to_string(),
            })
        } else {
            Err(BorsaError::AllProvidersFailed(errors))
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "borsa::core::fetch_single_latency",
            skip(self, call),
            fields(symbol = %inst.symbol(), capability = %capability_label, not_found = %not_found_label),
        )
    )]
    async fn fetch_single_latency<T, F, Fut>(
        &self,
        inst: &Instrument,
        capability_label: &'static str,
        not_found_label: &'static str,
        call: F,
    ) -> Result<T, BorsaError>
    where
        T: Send,
        F: Fn(Arc<dyn BorsaConnector>, Instrument) -> Option<Fut> + Clone + Send,
        Fut: core::future::Future<Output = Result<T, BorsaError>> + Send,
    {
        use futures::stream::{FuturesUnordered, StreamExt};

        let mut futs = FuturesUnordered::new();
        let mut attempted_any = false;
        for c in self.ordered(inst) {
            if let Some(fut) = call(c.clone(), inst.clone()) {
                let name = c.name();
                let timeout = self.cfg.provider_timeout;
                futs.push(async move {
                    (
                        name,
                        Self::provider_call_with_timeout(name, capability_label, timeout, fut)
                            .await,
                    )
                });
                attempted_any = true;
            }
        }

        let mut errors: Vec<BorsaError> = Vec::new();
        while let Some((name, res)) = futs.next().await {
            match res {
                Ok(v) => return Ok(v),
                Err(e @ (BorsaError::ProviderTimeout { .. } | BorsaError::NotFound { .. })) => {
                    errors.push(e);
                }
                Err(e) => errors.push(crate::core::tag_err(name, e)),
            }
        }

        if attempted_any {
            if !errors.is_empty()
                && errors
                    .iter()
                    .all(|e| matches!(e, BorsaError::ProviderTimeout { .. }))
            {
                Err(BorsaError::AllProvidersTimedOut {
                    capability: capability_label.to_string(),
                })
            } else if !errors.is_empty()
                && errors
                    .iter()
                    .all(|e| matches!(e, BorsaError::NotFound { .. }))
            {
                Err(BorsaError::not_found(format!(
                    "{} for {}",
                    not_found_label,
                    inst.symbol()
                )))
            } else {
                Err(BorsaError::AllProvidersFailed(errors))
            }
        } else {
            Err(BorsaError::unsupported(capability_label))
        }
    }
}
