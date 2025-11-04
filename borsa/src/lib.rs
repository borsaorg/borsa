//! Borsa orchestrates requests across multiple market data providers.
//!
//! Overview
//! - Routes requests to connectors that implement the `borsa_core` contracts.
//! - Applies per-symbol and per-kind priorities to influence provider order.
//! - Supports configurable fetch and merge strategies with resampling options.
//! - Normalizes error handling and exposes uniform domain types from `borsa_core`.
//!
//! Key behaviors and trade-offs
//! - Fetch strategy:
//!   - `PriorityWithFallback`: deterministic order, per-provider timeout, aggregates
//!     errors; fewer concurrent requests but potentially higher latency.
//!   - `Latency`: races eligible providers; lowest tail latency but higher request fanout.
//! - History merge:
//!   - `Deep`: fetch all eligible providers and backfill gaps; most complete series,
//!     more requests and provider load.
//!   - `Fallback`: first non-empty dataset wins; economical but can miss data present
//!     on lower-priority providers.
//! - Resampling: daily/weekly (or auto-subdaily→daily) simplifies downstream analysis
//!   but drops native cadence and clears per-candle `close_unadj` to avoid ambiguity.
//! - Adjusted preference: favors adjusted series to smooth corporate actions at the
//!   cost of diverging from unadjusted close values.
//! - Streaming: selects the first provider per asset kind that connects; supervised
//!   backoff with jitter reduces synchronized reconnect storms.
//!
//! Examples
//! Building an orchestrator with preferences and strategies:
//! ```rust,ignore
//! use std::sync::Arc;
//! use borsa::{Borsa, MergeStrategy, FetchStrategy, Resampling};
//! use borsa_core::AssetKind;
//!
//! let yf = Arc::new(YfConnector::new_default());
//! let av = Arc::new(AvConnector::new_with_key("..."));
//!
//! let routing = borsa_core::RoutingPolicyBuilder::new()
//!     .providers_for_kind(
//!         AssetKind::Equity,
//!         &[av.key(), yf.key()],
//!     )
//!     .build();
//!
//! let borsa = Borsa::builder()
//!     .with_connector(yf.clone())
//!     .with_connector(av.clone())
//!     // Type-safe, ergonomic API via typed connector keys
//!     .routing_policy(routing)
//!     .merge_history_strategy(MergeStrategy::Deep)
//!     .fetch_strategy(FetchStrategy::PriorityWithFallback)
//!     .resampling(Resampling::Daily)
//!     .build()?;
//! ```
//!
//! Fetching a quote and a merged history series:
//! ```rust,ignore
//! use borsa_core::{Instrument, Interval, Range};
//!
//! let aapl = Instrument::stock("AAPL");
//! let quote = borsa.quote(&aapl).await?;
//! let hist = borsa.history(
//!     &aapl,
//!     borsa_core::HistoryRequest{
//!         range: Some(Range::Y1),
//!         interval: Interval::D1,
//!         ..Default::default()
//!     }
//! ).await?;
//! ```
//!
//! Streaming with supervised failover:
//! ```rust,ignore
//! use borsa_core::Instrument;
//! let (handle, mut rx) = borsa.stream_quotes(&[Instrument::stock("AAPL")]).await?;
//! // ... consume updates ...
//! handle.stop().await;
//! ```
//!
//! Bulk download helper (multi-symbol history):
//! ```rust,ignore
//! use borsa_core::{Instrument, Interval, Range};
//! let report = borsa
//!     .download()
//!     .instruments(&[Instrument::stock("AAPL"), Instrument::stock("MSFT")])?
//!     .range(borsa_core::Range::M6)
//!     .interval(Interval::D1)
//!     .run()
//!     .await?;
//! if let Some(resp) = report.response.as_ref() {
//!     if let Some(aapl_history) = resp.series.get("AAPL") {
//!         // inspect candles returned for AAPL
//!     }
//! }
//! ```
//!
//! See `borsa/examples/` for runnable end-to-end demonstrations.
#![warn(missing_docs)]

pub(crate) mod core;
mod router;

pub use borsa_core::{
    Attribution, BackoffConfig, BorsaConfig, FetchStrategy, MergeStrategy, Resampling, Span,
};
pub use core::{Borsa, BorsaBuilder};
pub use router::download::DownloadBuilder;
pub use router::util::{collapse_errors, join_with_deadline};

pub use borsa_middleware::{QuotaMiddleware, CacheMiddleware, BlacklistMiddleware};

// Re-export core types for convenience
pub use borsa_core::{
    // Response types & Data Structures
    Address,
    // Foundational types
    AssetKind,
    BalanceSheetRow,
    BorsaError,
    Calendar,
    Candle,
    Capability,
    CashflowRow,
    CompanyProfile,
    Currency,
    DownloadEntry,
    DownloadReport,
    DownloadResponse,
    Earnings,
    EsgScores,
    Exchange,
    FastInfo,
    FundKind,
    FundProfile,
    // Request types
    HistoryRequest,
    HistoryRequestBuilder,
    HistoryResponse,
    IncomeStatementRow,
    Info,
    InfoReport,
    InsiderRosterHolder,
    InsiderTransaction,
    InstitutionalHolder,
    Instrument,
    Isin,
    IsoCurrency,
    MajorHolder,
    MarketState,
    Money,

    NetSharePurchaseActivity,
    NewsArticle,
    NewsRequest,
    OptionChain,
    OptionContract,
    PriceTarget,
    Profile,
    Quote,
    QuoteUpdate,
    RecommendationRow,
    RecommendationSummary,
    SearchReport,
    SearchRequest,

    SearchResult,
    UpgradeDowngradeRow,

    CacheConfig,
    QuotaConfig,
    QuotaConsumptionStrategy,
    QuotaState,
    
    BorsaConnector
};
