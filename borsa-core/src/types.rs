//! Re-export of foundational types from `paft` and aggregate/reporting types.
// Consolidated re-exports from paft so downstream crates can depend on `borsa-core` only

pub use paft::domain::{AssetKind, Exchange, Figi, Instrument, Isin, MarketState, Period, Symbol};

pub use paft::money::{
    Currency, ExchangeRate, IsoCurrency, Money, clear_currency_metadata, currency_metadata,
    set_currency_metadata, try_normalize_currency_code,
};

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
pub use paft::market::options::{OptionChain, OptionContract, OptionGreeks};
pub use paft::market::quote::{Quote, QuoteUpdate};
pub use paft::market::requests::history::{HistoryRequest, HistoryRequestBuilder, Interval, Range};
pub use paft::market::requests::news::{NewsRequest, NewsTab};
pub use paft::market::requests::search::SearchRequest;
pub use paft::market::responses::download::DownloadResponse;
pub use paft::market::responses::history::{Candle, HistoryMeta, HistoryResponse};
pub use paft::market::responses::search::{SearchResponse, SearchResult};

// Aggregates and reports from paft-aggregates (enabled via `paft/aggregates` feature)
pub use paft::aggregates::{DownloadReport, FastInfo, Info, InfoReport, SearchReport};
