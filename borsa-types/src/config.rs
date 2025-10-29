//! Configuration types shared across orchestrators and connectors.

// no extra prelude imports
use std::time::Duration;
use std::collections::HashMap;

use crate::routing_policy::RoutingPolicy;
use serde::{Deserialize, Serialize};

/// Strategy for selecting among eligible data providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub enum FetchStrategy {
    /// Use priority order and fall back to the next provider on failure.
    #[default]
    PriorityWithFallback,
    /// Race all eligible providers concurrently and return the first success.
    Latency,
}

/// Strategy for merging history data from multiple providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub enum MergeStrategy {
    /// Fetch from all eligible providers concurrently and merge their data.
    /// This produces the most complete dataset by backfilling gaps from lower-priority providers.
    #[default]
    Deep,
    /// Iterate through providers sequentially and stop as soon as one returns a non-empty dataset.
    /// This is more economical for API rate limits but may miss data from lower-priority providers.
    Fallback,
}

/// Forced resampling mode for merged history series.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Resampling {
    /// Do not force resampling; preserve the effective interval unless auto-subdaily triggers.
    #[default]
    None,
    /// Force resampling to daily cadence.
    Daily,
    /// Force resampling to weekly cadence.
    Weekly,
}

/// Strategy for consuming units from a quota when handling requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub enum QuotaConsumptionStrategy {
    /// Each request deducts exactly one unit from the quota budget.
    #[default]
    Unit,
    /// The caller specifies a weight (units) to deduct per request.
    /// This allows modeling provider-specific costs.
    Weighted,
    /// Evenly spread requests across hours within the window. This temporarily
    /// blocks once the per-hour allowance is exhausted, even if the daily budget
    /// still has remaining units. Intended to smooth provider load.
    EvenSpreadHourly,
}

/// Configuration for a token-like quota budget over a sliding window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaConfig {
    /// Maximum number of units that may be consumed within a single window.
    pub limit: u64,
    /// Duration of the accounting window.
    pub window: Duration,
    /// Strategy for how requests consume units from the budget.
    pub strategy: QuotaConsumptionStrategy,
}

impl Default for QuotaConfig {
    fn default() -> Self {
        Self {
            limit: 1000,
            window: Duration::from_secs(60),
            strategy: QuotaConsumptionStrategy::Unit,
        }
    }
}

/// Snapshot of a quota budget at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuotaState {
    /// Configured maximum units per window.
    pub limit: u64,
    /// Remaining units available in the current window.
    pub remaining: u64,
    /// Time remaining until the current window resets.
    pub reset_in: Duration,
}

/// Exponential backoff configuration for reconnecting streaming sessions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BackoffConfig {
    /// Minimum backoff delay in milliseconds.
    pub min_backoff_ms: u64,
    /// Maximum backoff delay in milliseconds.
    pub max_backoff_ms: u64,
    /// Exponential factor to increase delay after each failure (>= 1).
    pub factor: u32,
    /// Random jitter percentage [0, 100] added to each delay.
    pub jitter_percent: u8,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            min_backoff_ms: 500,
            max_backoff_ms: 30_000,
            factor: 2,
            jitter_percent: 20,
        }
    }
}

/// Global configuration for the `Borsa` orchestrator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BorsaConfig {
    /// Unified routing policy controlling provider and exchange ordering.
    ///
    /// - Provider rules select and order eligible connectors; unknown connector
    ///   keys are rejected during `borsa`'s build step.
    /// - Exchange preferences influence search de-duplication only.
    pub routing_policy: RoutingPolicy,
    /// Prefer adjusted history series when merging.
    pub prefer_adjusted_history: bool,
    /// Forced resampling mode for merged history.
    pub resampling: Resampling,
    /// If request interval is subdaily, resample to daily automatically.
    pub auto_resample_subdaily_to_daily: bool,
    /// Strategy for fetching from multiple providers.
    pub fetch_strategy: FetchStrategy,
    /// Strategy for merging history data from multiple providers.
    pub merge_history_strategy: MergeStrategy,
    /// Timeout for individual provider requests.
    pub provider_timeout: Duration,
    /// Optional overall request timeout for fan-out aggregations (e.g., history/search).
    /// If set, operations that aggregate multiple provider calls are bounded by this deadline.
    pub request_timeout: Option<Duration>,
    /// Optional backoff configuration used by streaming.
    pub backoff: Option<BackoffConfig>,
    /// When enabled, enforce per-symbol non-decreasing timestamps for stream updates.
    /// Updates with timestamps older than the last seen for the same symbol are dropped.
    /// Equal timestamps are allowed. Enabled by default.
    #[serde(default = "default_true")]
    pub stream_enforce_monotonic_timestamps: bool,
}

impl Default for BorsaConfig {
    fn default() -> Self {
        Self {
            routing_policy: RoutingPolicy::default(),
            prefer_adjusted_history: false,
            resampling: Resampling::None,
            auto_resample_subdaily_to_daily: false,
            fetch_strategy: FetchStrategy::default(),
            merge_history_strategy: MergeStrategy::default(),
            provider_timeout: Duration::from_secs(5),
            request_timeout: None,
            backoff: None,
            stream_enforce_monotonic_timestamps: true,
        }
    }
}

const fn default_true() -> bool {
    true
}

/// Configuration for per-capability response caching in middleware.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Default time-to-live (TTL) in milliseconds for all capabilities not explicitly overridden.
    ///
    /// TTL = 0 disables caching for that capability.
    pub default_ttl_ms: u64,
    /// Per-capability TTL overrides in milliseconds keyed by capability string (see `Capability::as_str()`).
    ///
    /// Example keys: "quote", "profile", "history", ...
    #[serde(default)]
    pub per_capability_ttl_ms: HashMap<String, u64>,

    /// Default max entries for each per-capability cache when not explicitly overridden.
    pub default_max_entries: usize,
    /// Per-capability max entries overrides keyed by capability string.
    #[serde(default)]
    pub per_capability_max_entries: HashMap<String, usize>,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            // Sensible default: 5 minutes
            default_ttl_ms: 5 * 60 * 1000,
            per_capability_ttl_ms: HashMap::new(),
            // Sensible default capacity per capability
            default_max_entries: 2000,
            per_capability_max_entries: HashMap::new(),
        }
    }
}

impl CacheConfig {
    /// Returns the TTL for a capability, or `None` if caching is disabled (TTL == 0).
    #[must_use]
    pub fn ttl_for(&self, cap: crate::Capability) -> Option<Duration> {
        let key = cap.as_str();
        let ms = self
            .per_capability_ttl_ms
            .get(key)
            .copied()
            .unwrap_or(self.default_ttl_ms);
        if ms == 0 { None } else { Some(Duration::from_millis(ms)) }
    }

    /// Returns the capacity for a capability, falling back to `default_max_entries`.
    #[must_use]
    pub fn capacity_for(&self, cap: crate::Capability) -> usize {
        let key = cap.as_str();
        self.per_capability_max_entries
            .get(key)
            .copied()
            .unwrap_or(self.default_max_entries)
    }
}
