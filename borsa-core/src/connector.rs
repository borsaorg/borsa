use async_trait::async_trait;

use crate::BorsaError;
use paft::domain::{AssetKind, Instrument, Isin};
use paft::fundamentals::analysis::{
    Earnings, PriceTarget, RecommendationRow, RecommendationSummary, UpgradeDowngradeRow,
};
use paft::fundamentals::esg::EsgScores;
use paft::fundamentals::holders::{
    InsiderRosterHolder, InsiderTransaction, InstitutionalHolder, MajorHolder,
    NetSharePurchaseActivity,
};
use paft::fundamentals::profile::Profile;
use paft::fundamentals::statements::{BalanceSheetRow, Calendar, CashflowRow, IncomeStatementRow};
use paft::market::news::NewsArticle;
use paft::market::options::OptionChain;
use paft::market::quote::{Quote, QuoteUpdate};
use paft::market::requests::history::{HistoryRequest, Interval};
use paft::market::requests::news::NewsRequest;
use paft::market::requests::search::SearchRequest;
use paft::market::responses::history::HistoryResponse;
use paft::market::responses::search::SearchResponse;

/// Typed key for identifying connectors in priority configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectorKey(pub &'static str);

impl ConnectorKey {
    /// Construct a new typed connector key from a static name.
    ///
    /// This is useful when configuring per-kind or per-symbol priorities.
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    /// Returns the inner static string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl From<ConnectorKey> for &'static str {
    fn from(k: ConnectorKey) -> Self {
        k.0
    }
}

/// Focused role trait for connectors that provide OHLCV history.
#[async_trait]
pub trait HistoryProvider: Send + Sync {
    /// Fetch OHLCV history and actions for the given instrument and request.
    async fn history(
        &self,
        instrument: &Instrument,
        req: HistoryRequest,
    ) -> Result<HistoryResponse, BorsaError>;

    /// REQUIRED: exact intervals this connector can natively serve for history.
    ///
    /// Parameters:
    /// - `kind`: asset kind to consider (some providers vary by kind).
    ///
    /// Returns the static slice of supported `Interval`s.
    fn supported_history_intervals(&self, kind: AssetKind) -> &'static [Interval];
}

/// Focused role trait for connectors that provide quotes.
#[async_trait]
pub trait QuoteProvider: Send + Sync {
    /// Fetch a point-in-time quote for the given instrument.
    async fn quote(&self, instrument: &Instrument) -> Result<Quote, BorsaError>;
}

// Granular role traits
/// Focused role trait for connectors that provide earnings fundamentals.
#[async_trait]
pub trait EarningsProvider: Send + Sync {
    /// Fetch earnings for the given instrument.
    async fn earnings(&self, instrument: &Instrument) -> Result<Earnings, BorsaError>;
}

/// Focused role trait for connectors that provide income statements.
#[async_trait]
pub trait IncomeStatementProvider: Send + Sync {
    /// Fetch income statement rows for the given instrument.
    async fn income_statement(
        &self,
        instrument: &Instrument,
        quarterly: bool,
    ) -> Result<Vec<IncomeStatementRow>, BorsaError>;
}

/// Focused role trait for connectors that provide balance sheets.
#[async_trait]
pub trait BalanceSheetProvider: Send + Sync {
    /// Fetch balance sheet rows for the given instrument.
    async fn balance_sheet(
        &self,
        instrument: &Instrument,
        quarterly: bool,
    ) -> Result<Vec<BalanceSheetRow>, BorsaError>;
}

/// Focused role trait for connectors that provide cashflow statements.
#[async_trait]
pub trait CashflowProvider: Send + Sync {
    /// Fetch cashflow rows for the given instrument.
    async fn cashflow(
        &self,
        instrument: &Instrument,
        quarterly: bool,
    ) -> Result<Vec<CashflowRow>, BorsaError>;
}

/// Focused role trait for connectors that provide corporate event calendars.
#[async_trait]
pub trait CalendarProvider: Send + Sync {
    /// Fetch the fundamentals calendar for the given instrument.
    async fn calendar(&self, instrument: &Instrument) -> Result<Calendar, BorsaError>;
}

/// Focused role trait for connectors that provide analyst recommendations.
#[async_trait]
pub trait RecommendationsProvider: Send + Sync {
    /// Fetch recommendation rows for the given instrument.
    async fn recommendations(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<RecommendationRow>, BorsaError>;
}

/// Focused role trait for connectors that provide an aggregate recommendations summary.
#[async_trait]
pub trait RecommendationsSummaryProvider: Send + Sync {
    /// Fetch a recommendations summary for the given instrument.
    async fn recommendations_summary(
        &self,
        instrument: &Instrument,
    ) -> Result<RecommendationSummary, BorsaError>;
}

/// Focused role trait for connectors that provide upgrades/downgrades history.
#[async_trait]
pub trait UpgradesDowngradesProvider: Send + Sync {
    /// Fetch upgrade/downgrade rows for the given instrument.
    async fn upgrades_downgrades(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<UpgradeDowngradeRow>, BorsaError>;
}

/// Focused role trait for connectors that provide analyst price targets.
#[async_trait]
pub trait AnalystPriceTargetProvider: Send + Sync {
    /// Fetch the current analyst price target for the given instrument.
    async fn analyst_price_target(
        &self,
        instrument: &Instrument,
    ) -> Result<PriceTarget, BorsaError>;
}

/// Focused role trait for connectors that provide major holder breakdowns.
#[async_trait]
pub trait MajorHoldersProvider: Send + Sync {
    /// Fetch major holder rows for the given instrument.
    async fn major_holders(&self, instrument: &Instrument) -> Result<Vec<MajorHolder>, BorsaError>;
}

/// Focused role trait for connectors that provide institutional holders.
#[async_trait]
pub trait InstitutionalHoldersProvider: Send + Sync {
    /// Fetch institutional holder rows for the given instrument.
    async fn institutional_holders(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<InstitutionalHolder>, BorsaError>;
}

/// Focused role trait for connectors that provide mutual fund holders.
#[async_trait]
pub trait MutualFundHoldersProvider: Send + Sync {
    /// Fetch mutual fund holder rows for the given instrument.
    async fn mutual_fund_holders(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<InstitutionalHolder>, BorsaError>;
}

/// Focused role trait for connectors that provide insider transaction events.
#[async_trait]
pub trait InsiderTransactionsProvider: Send + Sync {
    /// Fetch insider transactions for the given instrument.
    async fn insider_transactions(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<InsiderTransaction>, BorsaError>;
}

/// Focused role trait for connectors that provide insider roster holders.
#[async_trait]
pub trait InsiderRosterHoldersProvider: Send + Sync {
    /// Fetch insider roster holders for the given instrument.
    async fn insider_roster_holders(
        &self,
        instrument: &Instrument,
    ) -> Result<Vec<InsiderRosterHolder>, BorsaError>;
}

/// Focused role trait for connectors that provide net share purchase activity.
#[async_trait]
pub trait NetSharePurchaseActivityProvider: Send + Sync {
    /// Fetch net share purchase activity for the given instrument, if any.
    async fn net_share_purchase_activity(
        &self,
        instrument: &Instrument,
    ) -> Result<Option<NetSharePurchaseActivity>, BorsaError>;
}

/// Focused role trait for connectors that provide company profile data.
#[async_trait]
pub trait ProfileProvider: Send + Sync {
    /// Fetch a profile for the given instrument.
    async fn profile(&self, instrument: &Instrument) -> Result<Profile, BorsaError>;
}

/// Focused role trait for connectors that provide ISIN lookup.
#[async_trait]
pub trait IsinProvider: Send + Sync {
    /// Fetch an ISIN string for the given instrument, if available.
    async fn isin(&self, instrument: &Instrument) -> Result<Option<Isin>, BorsaError>;
}

/// Focused role trait for connectors that can search instruments.
#[async_trait]
pub trait SearchProvider: Send + Sync {
    /// Perform a symbol search according to the provided request.
    async fn search(&self, req: SearchRequest) -> Result<SearchResponse, BorsaError>;
}

/// Focused role trait for connectors that provide ESG scores.
#[async_trait]
pub trait EsgProvider: Send + Sync {
    /// Fetch ESG scores for the given instrument.
    async fn sustainability(&self, instrument: &Instrument) -> Result<EsgScores, BorsaError>;
}

/// Focused role trait for connectors that provide news articles.
#[async_trait]
pub trait NewsProvider: Send + Sync {
    /// Fetch news articles for the given instrument.
    async fn news(
        &self,
        instrument: &Instrument,
        req: NewsRequest,
    ) -> Result<Vec<NewsArticle>, BorsaError>;
}

/// Focused role trait for connectors that provide streaming quote updates.
#[async_trait]
pub trait StreamProvider: Send + Sync {
    /// Start a streaming session for the given instruments.
    async fn stream_quotes(
        &self,
        instruments: &[Instrument],
    ) -> Result<
        (
            crate::stream::StreamHandle,
            tokio::sync::mpsc::Receiver<QuoteUpdate>,
        ),
        BorsaError,
    >;
}

/// Focused role trait for connectors that provide options expirations.
#[async_trait]
pub trait OptionsExpirationsProvider: Send + Sync {
    /// Fetch a list of option expiration timestamps for the given instrument.
    async fn options_expirations(&self, instrument: &Instrument) -> Result<Vec<i64>, BorsaError>;
}

/// Focused role trait for connectors that provide option chains.
#[async_trait]
pub trait OptionChainProvider: Send + Sync {
    /// Fetch an option chain for the given instrument and optional expiration date.
    async fn option_chain(
        &self,
        instrument: &Instrument,
        date: Option<i64>,
    ) -> Result<OptionChain, BorsaError>;
}

/// Main connector trait implemented by provider crates. Exposes capability discovery.
#[async_trait]
pub trait BorsaConnector: Send + Sync {
    /// A stable identifier for priority lists (e.g., "borsa-yfinance", "borsa-coinmarketcap").
    fn name(&self) -> &'static str;

    /// Human-friendly vendor string.
    fn vendor(&self) -> &'static str {
        "unknown"
    }

    /// Whether this connector *claims* to support a given asset kind.
    ///
    /// Default: returns `false` for all kinds. Connectors must explicitly override
    /// this method to declare which asset kinds they support.
    fn supports_kind(&self, kind: AssetKind) -> bool {
        let _ = kind;
        false
    }

    /// Advertise history capability by returning a usable trait object reference when supported.
    fn as_history_provider(&self) -> Option<&dyn HistoryProvider> {
        None
    }

    /// Advertise quote capability by returning a usable trait object reference when supported.
    fn as_quote_provider(&self) -> Option<&dyn QuoteProvider> {
        None
    }

    /// If implemented, returns a trait object for earnings fundamentals.
    fn as_earnings_provider(&self) -> Option<&dyn EarningsProvider> {
        None
    }
    /// If implemented, returns a trait object for income statements.
    fn as_income_statement_provider(&self) -> Option<&dyn IncomeStatementProvider> {
        None
    }
    /// If implemented, returns a trait object for balance sheets.
    fn as_balance_sheet_provider(&self) -> Option<&dyn BalanceSheetProvider> {
        None
    }
    /// If implemented, returns a trait object for cashflow statements.
    fn as_cashflow_provider(&self) -> Option<&dyn CashflowProvider> {
        None
    }
    /// If implemented, returns a trait object for the fundamentals calendar.
    fn as_calendar_provider(&self) -> Option<&dyn CalendarProvider> {
        None
    }

    /// If implemented, returns a trait object for analyst recommendations.
    fn as_recommendations_provider(&self) -> Option<&dyn RecommendationsProvider> {
        None
    }
    /// If implemented, returns a trait object for recommendations summary.
    fn as_recommendations_summary_provider(&self) -> Option<&dyn RecommendationsSummaryProvider> {
        None
    }
    /// If implemented, returns a trait object for upgrades/downgrades.
    fn as_upgrades_downgrades_provider(&self) -> Option<&dyn UpgradesDowngradesProvider> {
        None
    }
    /// If implemented, returns a trait object for analyst price targets.
    fn as_analyst_price_target_provider(&self) -> Option<&dyn AnalystPriceTargetProvider> {
        None
    }

    /// If implemented, returns a trait object for major holders.
    fn as_major_holders_provider(&self) -> Option<&dyn MajorHoldersProvider> {
        None
    }
    /// If implemented, returns a trait object for institutional holders.
    fn as_institutional_holders_provider(&self) -> Option<&dyn InstitutionalHoldersProvider> {
        None
    }
    /// If implemented, returns a trait object for mutual fund holders.
    fn as_mutual_fund_holders_provider(&self) -> Option<&dyn MutualFundHoldersProvider> {
        None
    }
    /// If implemented, returns a trait object for insider transactions.
    fn as_insider_transactions_provider(&self) -> Option<&dyn InsiderTransactionsProvider> {
        None
    }
    /// If implemented, returns a trait object for insider roster holders.
    fn as_insider_roster_holders_provider(&self) -> Option<&dyn InsiderRosterHoldersProvider> {
        None
    }
    /// If implemented, returns a trait object for net share purchase activity.
    fn as_net_share_purchase_activity_provider(
        &self,
    ) -> Option<&dyn NetSharePurchaseActivityProvider> {
        None
    }

    /// If implemented, returns a trait object for company profiles.
    fn as_profile_provider(&self) -> Option<&dyn ProfileProvider> {
        None
    }
    /// If implemented, returns a trait object for ISIN lookup.
    fn as_isin_provider(&self) -> Option<&dyn IsinProvider> {
        None
    }
    /// If implemented, returns a trait object for instrument search.
    fn as_search_provider(&self) -> Option<&dyn SearchProvider> {
        None
    }
    /// If implemented, returns a trait object for ESG scores.
    fn as_esg_provider(&self) -> Option<&dyn EsgProvider> {
        None
    }
    /// If implemented, returns a trait object for news articles.
    fn as_news_provider(&self) -> Option<&dyn NewsProvider> {
        None
    }

    /// If implemented, returns a trait object for options expirations.
    fn as_options_expirations_provider(&self) -> Option<&dyn OptionsExpirationsProvider> {
        None
    }
    /// If implemented, returns a trait object for option chains.
    fn as_option_chain_provider(&self) -> Option<&dyn OptionChainProvider> {
        None
    }

    /// If implemented, returns a trait object for quote streaming.
    fn as_stream_provider(&self) -> Option<&dyn StreamProvider> {
        None
    }
}
