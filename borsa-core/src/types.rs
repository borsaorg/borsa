//! Re-export of foundational types from `paft` and `borsa-types`.
// Consolidated re-exports so downstream crates can depend on `borsa-core` only

// Aggregates, config, and reports (FastInfo/Info from `paft`, report envelopes from `borsa-types`)
pub use borsa_types::{BorsaError, Capability};

pub use borsa_types::ConnectorKey;
pub use borsa_types::routing_policy::Selector;
pub use borsa_types::{
    Attribution, BackoffConfig, BorsaConfig, DownloadReport, FetchStrategy, InfoReport,
    MergeStrategy, Resampling, SearchReport, Span,
};
pub use borsa_types::{CacheConfig, QuotaConfig, QuotaConsumptionStrategy, QuotaState};
pub use borsa_types::{Preference, RoutingContext, RoutingPolicy, RoutingPolicyBuilder, ScopeKey};

pub use paft::domain::{
    AssetKind, EventID, Exchange, Figi, IdentifierScheme, Instrument, Isin, MarketState, OutcomeID,
    Period, PredictionID, SecurityId, Symbol,
};

pub use paft::money::{
    Currency, ExchangeRate, IsoCurrency, Money, clear_currency_metadata, currency_metadata,
    set_currency_metadata, try_normalize_currency_code,
};

pub use paft::{Decimal, RoundingStrategy};

pub use paft::fundamentals::analysis::{
    AnalysisSummary, Earnings, EarningsQuarter, EarningsQuarterEps, EarningsTrendRow, EarningsYear,
    PriceTarget, RecommendationAction, RecommendationGrade, RecommendationRow,
    RecommendationSummary, UpgradeDowngradeRow,
};
pub use paft::fundamentals::esg::{EsgInvolvement, EsgScores, EsgSummary};
pub use paft::fundamentals::holders::{
    InsiderPosition, InsiderRosterHolder, InsiderTransaction, InstitutionalHolder, MajorHolder,
    NetSharePurchaseActivity, TransactionType,
};
pub use paft::fundamentals::profile::{
    Address, CompanyProfile, FundKind, FundProfile, Profile, ShareCount,
};
pub use paft::fundamentals::statements::{
    BalanceSheetRow, Calendar, CashflowRow, IncomeStatementRow,
};

pub use paft::market::action::Action;
pub use paft::market::news::NewsArticle;
pub use paft::market::options::{OptionChain, OptionContract, OptionGreeks, OptionUpdate};
pub use paft::market::quote::{Quote, QuoteUpdate};
pub use paft::market::requests::history::{HistoryRequest, HistoryRequestBuilder, Interval, Range};
pub use paft::market::requests::news::{NewsRequest, NewsTab};
pub use paft::market::requests::search::SearchRequest;
pub use paft::market::responses::download::{DownloadEntry, DownloadResponse};
pub use paft::market::responses::history::CandleUpdate;
pub use paft::market::responses::history::{Candle, HistoryMeta, HistoryResponse};
pub use paft::market::responses::search::{SearchResponse, SearchResult};

pub use paft::aggregates::{FastInfo, Info};

// Optional: re-export DataFrame conversion trait when the feature is enabled
#[cfg(feature = "dataframe")]
pub use paft::core::dataframe::{ToDataFrame, ToDataFrameVec};
