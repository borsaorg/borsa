//! Configuration types shared across orchestrators and connectors.

use std::collections::HashMap;
use std::time::Duration;

use crate::connector::ConnectorKey;
use paft::domain::AssetKind;

/// Strategy for selecting among eligible data providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FetchStrategy {
    /// Use priority order and fall back to the next provider on failure.
    #[default]
    PriorityWithFallback,
    /// Race all eligible providers concurrently and return the first success.
    Latency,
}

/// Strategy for merging history data from multiple providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Resampling {
    /// Do not force resampling; preserve the effective interval unless auto-subdaily triggers.
    #[default]
    None,
    /// Force resampling to daily cadence.
    Daily,
    /// Force resampling to weekly cadence.
    Weekly,
}

/// Exponential backoff configuration for reconnecting streaming sessions.
#[derive(Debug, Clone, Copy)]
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
#[derive(Debug, Clone)]
pub struct BorsaConfig {
    /// Preferred provider order per asset kind (highest priority first).
    pub per_kind_priority: HashMap<AssetKind, Vec<ConnectorKey>>,
    /// Preferred provider order per specific symbol.
    pub per_symbol_priority: HashMap<String, Vec<ConnectorKey>>,
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
}

impl Default for BorsaConfig {
    fn default() -> Self {
        Self {
            per_kind_priority: HashMap::new(),
            per_symbol_priority: HashMap::new(),
            prefer_adjusted_history: false,
            resampling: Resampling::None,
            auto_resample_subdaily_to_daily: false,
            fetch_strategy: FetchStrategy::default(),
            merge_history_strategy: MergeStrategy::default(),
            provider_timeout: Duration::from_secs(5),
            request_timeout: None,
            backoff: None,
        }
    }
}
