use core::fmt;
use serde::{Deserialize, Serialize};

/// High-level capability labels for routing, errors, and telemetry.
///
/// These map one-to-one with router endpoints and allow consistent
/// Display formatting and match-exhaustive handling when adding
/// new capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Capability {
    /// Point-in-time quote for a single instrument.
    Quote,
    /// Free-text instrument search.
    Search,

    /// Historical OHLCV candles and actions.
    History,
    /// Bulk download of history across instruments.
    DownloadHistory,

    /// Company or fund profile.
    Profile,
    /// ISIN resolution.
    Isin,

    /// Fundamentals: earnings datasets.
    Earnings,
    /// Fundamentals: income statement rows.
    IncomeStatement,
    /// Fundamentals: balance sheet rows.
    BalanceSheet,
    /// Fundamentals: cashflow rows.
    Cashflow,
    /// Fundamentals: corporate calendar (earnings dates, dividends).
    Calendar,

    /// Analysis: detailed recommendations.
    Recommendations,
    /// Analysis: summary of recommendations.
    RecommendationsSummary,
    /// Analysis: broker upgrades and downgrades.
    UpgradesDowngrades,
    /// Analysis: analyst price target snapshot.
    AnalystPriceTarget,

    /// Holders: major holder percentages.
    MajorHolders,
    /// Holders: institutional holders.
    InstitutionalHolders,
    /// Holders: mutual fund holders.
    MutualFundHolders,
    /// Holders: insider transactions.
    InsiderTransactions,
    /// Holders: insider roster.
    InsiderRoster,
    /// Holders: net share purchase activity summary.
    NetSharePurchaseActivity,

    /// ESG sustainability scores and flags.
    Esg,
    /// Recent news articles for an instrument.
    News,

    /// Options: expirations list.
    OptionsExpirations,
    /// Options: option chain for an expiration date.
    OptionChain,

    /// Streaming: quotes stream.
    StreamQuotes,
    /// Streaming: candle stream.
    StreamCandles,
    /// Streaming: options stream.
    StreamOptions,
}

impl Capability {
    /// Stable, kebab-case identifier for logs/errors.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Quote => "quote",
            Self::Search => "search",
            Self::History => "history",
            Self::DownloadHistory => "download:history",
            Self::Profile => "profile",
            Self::Isin => "isin",
            Self::Earnings => "earnings",
            Self::IncomeStatement => "income-statement",
            Self::BalanceSheet => "balance-sheet",
            Self::Cashflow => "cashflow",
            Self::Calendar => "calendar",
            Self::Recommendations => "recommendations",
            Self::RecommendationsSummary => "recommendations-summary",
            Self::UpgradesDowngrades => "upgrades-downgrades",
            Self::AnalystPriceTarget => "analyst-price-target",
            Self::MajorHolders => "major-holders",
            Self::InstitutionalHolders => "institutional-holders",
            Self::MutualFundHolders => "mutual-fund-holders",
            Self::InsiderTransactions => "insider-transactions",
            Self::InsiderRoster => "insider-roster",
            Self::NetSharePurchaseActivity => "net-share-purchase-activity",
            Self::Esg => "esg",
            Self::News => "news",
            Self::OptionsExpirations => "options-expirations",
            Self::OptionChain => "option-chain",
            Self::StreamQuotes => "stream-quotes",
            Self::StreamCandles => "stream-candles",
            Self::StreamOptions => "stream-options",
        }
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
