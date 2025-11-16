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
//! - Basic quote: see `./examples/01_simple_quote.rs`.
//! - Streaming with supervised failover: see `./examples/17_streaming.rs`.
//! - Bulk download: see `./examples/21_download_builder.rs`.
//! - More examples in `./examples/`.
#![warn(missing_docs)]

pub(crate) mod core;
mod router;

pub use borsa_core::{
    Attribution, BackoffConfig, BorsaConfig, FetchStrategy, MergeStrategy, Resampling, Span,
};
pub use core::{Borsa, BorsaBuilder};
pub use router::download::DownloadBuilder;
pub use router::util::{collapse_errors, join_with_deadline};

pub use borsa_middleware::{BlacklistMiddleware, CacheMiddleware, QuotaMiddleware};

// Re-export core types for convenience
pub use borsa_core::{
    // Response types & Data Structures
    Address,
    // Foundational types
    AssetKind,
    BalanceSheetRow,
    BorsaConnector,
    BorsaError,
    CacheConfig,
    Calendar,
    Candle,
    CandleUpdate,
    Capability,
    CashflowRow,
    CompanyProfile,
    Currency,
    Decimal,
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
    Interval,
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
    OptionUpdate,
    PriceTarget,
    Profile,
    QuotaConfig,
    QuotaConsumptionStrategy,
    QuotaState,

    Quote,
    QuoteUpdate,
    Range,
    RecommendationRow,
    RecommendationSummary,
    RoundingStrategy,
    SearchReport,
    SearchRequest,

    SearchResult,
    UpgradeDowngradeRow,
};
