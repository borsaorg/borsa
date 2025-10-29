use async_trait::async_trait;

use crate::BorsaError;
pub use borsa_types::ConnectorKey;
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

    /// Canonical connector key constructed from the static name.
    ///
    /// Use this helper when configuring routing policies.
    fn key(&self) -> ConnectorKey {
        ConnectorKey::new(self.name())
    }

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

/// Generate `as_*_provider` accessors for a wrapper that implements
/// `BorsaConnector` by delegating to an inner field.
#[macro_export]
macro_rules! borsa_connector_accessors {
    ($inner:ident) => {
        fn as_history_provider(&self) -> Option<&dyn $crate::connector::HistoryProvider> {
            if self.$inner.as_history_provider().is_some() {
                Some(self as &dyn $crate::connector::HistoryProvider)
            } else {
                None
            }
        }
        fn as_quote_provider(&self) -> Option<&dyn $crate::connector::QuoteProvider> {
            if self.$inner.as_quote_provider().is_some() {
                Some(self as &dyn $crate::connector::QuoteProvider)
            } else {
                None
            }
        }
        fn as_earnings_provider(&self) -> Option<&dyn $crate::connector::EarningsProvider> {
            if self.$inner.as_earnings_provider().is_some() {
                Some(self as &dyn $crate::connector::EarningsProvider)
            } else {
                None
            }
        }
        fn as_income_statement_provider(
            &self,
        ) -> Option<&dyn $crate::connector::IncomeStatementProvider> {
            if self.$inner.as_income_statement_provider().is_some() {
                Some(self as &dyn $crate::connector::IncomeStatementProvider)
            } else {
                None
            }
        }
        fn as_balance_sheet_provider(
            &self,
        ) -> Option<&dyn $crate::connector::BalanceSheetProvider> {
            if self.$inner.as_balance_sheet_provider().is_some() {
                Some(self as &dyn $crate::connector::BalanceSheetProvider)
            } else {
                None
            }
        }
        fn as_cashflow_provider(&self) -> Option<&dyn $crate::connector::CashflowProvider> {
            if self.$inner.as_cashflow_provider().is_some() {
                Some(self as &dyn $crate::connector::CashflowProvider)
            } else {
                None
            }
        }
        fn as_calendar_provider(&self) -> Option<&dyn $crate::connector::CalendarProvider> {
            if self.$inner.as_calendar_provider().is_some() {
                Some(self as &dyn $crate::connector::CalendarProvider)
            } else {
                None
            }
        }
        fn as_recommendations_provider(
            &self,
        ) -> Option<&dyn $crate::connector::RecommendationsProvider> {
            if self.$inner.as_recommendations_provider().is_some() {
                Some(self as &dyn $crate::connector::RecommendationsProvider)
            } else {
                None
            }
        }
        fn as_recommendations_summary_provider(
            &self,
        ) -> Option<&dyn $crate::connector::RecommendationsSummaryProvider> {
            if self.$inner.as_recommendations_summary_provider().is_some() {
                Some(self as &dyn $crate::connector::RecommendationsSummaryProvider)
            } else {
                None
            }
        }
        fn as_upgrades_downgrades_provider(
            &self,
        ) -> Option<&dyn $crate::connector::UpgradesDowngradesProvider> {
            if self.$inner.as_upgrades_downgrades_provider().is_some() {
                Some(self as &dyn $crate::connector::UpgradesDowngradesProvider)
            } else {
                None
            }
        }
        fn as_analyst_price_target_provider(
            &self,
        ) -> Option<&dyn $crate::connector::AnalystPriceTargetProvider> {
            if self.$inner.as_analyst_price_target_provider().is_some() {
                Some(self as &dyn $crate::connector::AnalystPriceTargetProvider)
            } else {
                None
            }
        }
        fn as_major_holders_provider(
            &self,
        ) -> Option<&dyn $crate::connector::MajorHoldersProvider> {
            if self.$inner.as_major_holders_provider().is_some() {
                Some(self as &dyn $crate::connector::MajorHoldersProvider)
            } else {
                None
            }
        }
        fn as_institutional_holders_provider(
            &self,
        ) -> Option<&dyn $crate::connector::InstitutionalHoldersProvider> {
            if self.$inner.as_institutional_holders_provider().is_some() {
                Some(self as &dyn $crate::connector::InstitutionalHoldersProvider)
            } else {
                None
            }
        }
        fn as_mutual_fund_holders_provider(
            &self,
        ) -> Option<&dyn $crate::connector::MutualFundHoldersProvider> {
            if self.$inner.as_mutual_fund_holders_provider().is_some() {
                Some(self as &dyn $crate::connector::MutualFundHoldersProvider)
            } else {
                None
            }
        }
        fn as_insider_transactions_provider(
            &self,
        ) -> Option<&dyn $crate::connector::InsiderTransactionsProvider> {
            if self.$inner.as_insider_transactions_provider().is_some() {
                Some(self as &dyn $crate::connector::InsiderTransactionsProvider)
            } else {
                None
            }
        }
        fn as_insider_roster_holders_provider(
            &self,
        ) -> Option<&dyn $crate::connector::InsiderRosterHoldersProvider> {
            if self.$inner.as_insider_roster_holders_provider().is_some() {
                Some(self as &dyn $crate::connector::InsiderRosterHoldersProvider)
            } else {
                None
            }
        }
        fn as_net_share_purchase_activity_provider(
            &self,
        ) -> Option<&dyn $crate::connector::NetSharePurchaseActivityProvider> {
            if self
                .$inner
                .as_net_share_purchase_activity_provider()
                .is_some()
            {
                Some(self as &dyn $crate::connector::NetSharePurchaseActivityProvider)
            } else {
                None
            }
        }
        fn as_profile_provider(&self) -> Option<&dyn $crate::connector::ProfileProvider> {
            if self.$inner.as_profile_provider().is_some() {
                Some(self as &dyn $crate::connector::ProfileProvider)
            } else {
                None
            }
        }
        fn as_isin_provider(&self) -> Option<&dyn $crate::connector::IsinProvider> {
            if self.$inner.as_isin_provider().is_some() {
                Some(self as &dyn $crate::connector::IsinProvider)
            } else {
                None
            }
        }
        fn as_search_provider(&self) -> Option<&dyn $crate::connector::SearchProvider> {
            if self.$inner.as_search_provider().is_some() {
                Some(self as &dyn $crate::connector::SearchProvider)
            } else {
                None
            }
        }
        fn as_esg_provider(&self) -> Option<&dyn $crate::connector::EsgProvider> {
            if self.$inner.as_esg_provider().is_some() {
                Some(self as &dyn $crate::connector::EsgProvider)
            } else {
                None
            }
        }
        fn as_news_provider(&self) -> Option<&dyn $crate::connector::NewsProvider> {
            if self.$inner.as_news_provider().is_some() {
                Some(self as &dyn $crate::connector::NewsProvider)
            } else {
                None
            }
        }
        fn as_options_expirations_provider(
            &self,
        ) -> Option<&dyn $crate::connector::OptionsExpirationsProvider> {
            if self.$inner.as_options_expirations_provider().is_some() {
                Some(self as &dyn $crate::connector::OptionsExpirationsProvider)
            } else {
                None
            }
        }
        fn as_option_chain_provider(&self) -> Option<&dyn $crate::connector::OptionChainProvider> {
            if self.$inner.as_option_chain_provider().is_some() {
                Some(self as &dyn $crate::connector::OptionChainProvider)
            } else {
                None
            }
        }
        fn as_stream_provider(&self) -> Option<&dyn $crate::connector::StreamProvider> {
            if self.$inner.as_stream_provider().is_some() {
                Some(self as &dyn $crate::connector::StreamProvider)
            } else {
                None
            }
        }
    };
}

/// Generate all provider trait impls for a wrapper type `$self_ty`, delegating
/// to an inner field `$inner` and applying middleware hooks.
#[macro_export]
macro_rules! borsa_delegate_provider_impls {
    ($self_ty:ty, $inner:ident) => {
        #[async_trait::async_trait]
        impl $crate::connector::HistoryProvider for $self_ty {
            async fn history(
                &self,
                instrument: &$crate::Instrument,
                req: $crate::HistoryRequest,
            ) -> Result<$crate::HistoryResponse, $crate::BorsaError> {
                let ctx = $crate::middleware::CallContext::new($crate::Capability::History);
                <Self as $crate::Middleware>::pre_call(self, &ctx).await?;
                let inner = self
                    .$inner
                    .as_history_provider()
                    .ok_or_else(|| $crate::BorsaError::unsupported("history"))?;
                inner
                    .history(instrument, req)
                    .await
                    .map_err(|e| <Self as $crate::Middleware>::map_error(self, e, &ctx))
            }
            fn supported_history_intervals(
                &self,
                kind: $crate::AssetKind,
            ) -> &'static [$crate::Interval] {
                if let Some(inner) = self.$inner.as_history_provider() {
                    inner.supported_history_intervals(kind)
                } else {
                    &[]
                }
            }
        }

        #[async_trait::async_trait]
        impl $crate::connector::QuoteProvider for $self_ty {
            async fn quote(
                &self,
                instrument: &$crate::Instrument,
            ) -> Result<$crate::Quote, $crate::BorsaError> {
                let ctx = $crate::middleware::CallContext::new($crate::Capability::Quote);
                <Self as $crate::Middleware>::pre_call(self, &ctx).await?;
                let inner = self
                    .$inner
                    .as_quote_provider()
                    .ok_or_else(|| $crate::BorsaError::unsupported("quote"))?;
                inner
                    .quote(instrument)
                    .await
                    .map_err(|e| <Self as $crate::Middleware>::map_error(self, e, &ctx))
            }
        }

        #[async_trait::async_trait]
        impl $crate::connector::EarningsProvider for $self_ty {
            async fn earnings(
                &self,
                instrument: &$crate::Instrument,
            ) -> Result<$crate::Earnings, $crate::BorsaError> {
                let ctx = $crate::middleware::CallContext::new($crate::Capability::Earnings);
                <Self as $crate::Middleware>::pre_call(self, &ctx).await?;
                let inner = self
                    .$inner
                    .as_earnings_provider()
                    .ok_or_else(|| $crate::BorsaError::unsupported("earnings"))?;
                inner
                    .earnings(instrument)
                    .await
                    .map_err(|e| <Self as $crate::Middleware>::map_error(self, e, &ctx))
            }
        }

        #[async_trait::async_trait]
        impl $crate::connector::IncomeStatementProvider for $self_ty {
            async fn income_statement(
                &self,
                instrument: &$crate::Instrument,
                quarterly: bool,
            ) -> Result<Vec<$crate::IncomeStatementRow>, $crate::BorsaError> {
                let ctx = $crate::middleware::CallContext::new($crate::Capability::IncomeStatement);
                <Self as $crate::Middleware>::pre_call(self, &ctx).await?;
                let inner = self
                    .$inner
                    .as_income_statement_provider()
                    .ok_or_else(|| $crate::BorsaError::unsupported("income_statement"))?;
                inner
                    .income_statement(instrument, quarterly)
                    .await
                    .map_err(|e| <Self as $crate::Middleware>::map_error(self, e, &ctx))
            }
        }

        #[async_trait::async_trait]
        impl $crate::connector::BalanceSheetProvider for $self_ty {
            async fn balance_sheet(
                &self,
                instrument: &$crate::Instrument,
                quarterly: bool,
            ) -> Result<Vec<$crate::BalanceSheetRow>, $crate::BorsaError> {
                let ctx = $crate::middleware::CallContext::new($crate::Capability::BalanceSheet);
                <Self as $crate::Middleware>::pre_call(self, &ctx).await?;
                let inner = self
                    .$inner
                    .as_balance_sheet_provider()
                    .ok_or_else(|| $crate::BorsaError::unsupported("balance_sheet"))?;
                inner
                    .balance_sheet(instrument, quarterly)
                    .await
                    .map_err(|e| <Self as $crate::Middleware>::map_error(self, e, &ctx))
            }
        }

        #[async_trait::async_trait]
        impl $crate::connector::CashflowProvider for $self_ty {
            async fn cashflow(
                &self,
                instrument: &$crate::Instrument,
                quarterly: bool,
            ) -> Result<Vec<$crate::CashflowRow>, $crate::BorsaError> {
                let ctx = $crate::middleware::CallContext::new($crate::Capability::Cashflow);
                <Self as $crate::Middleware>::pre_call(self, &ctx).await?;
                let inner = self
                    .$inner
                    .as_cashflow_provider()
                    .ok_or_else(|| $crate::BorsaError::unsupported("cashflow"))?;
                inner
                    .cashflow(instrument, quarterly)
                    .await
                    .map_err(|e| <Self as $crate::Middleware>::map_error(self, e, &ctx))
            }
        }

        #[async_trait::async_trait]
        impl $crate::connector::CalendarProvider for $self_ty {
            async fn calendar(
                &self,
                instrument: &$crate::Instrument,
            ) -> Result<$crate::Calendar, $crate::BorsaError> {
                let ctx = $crate::middleware::CallContext::new($crate::Capability::Calendar);
                <Self as $crate::Middleware>::pre_call(self, &ctx).await?;
                let inner = self
                    .$inner
                    .as_calendar_provider()
                    .ok_or_else(|| $crate::BorsaError::unsupported("calendar"))?;
                inner
                    .calendar(instrument)
                    .await
                    .map_err(|e| <Self as $crate::Middleware>::map_error(self, e, &ctx))
            }
        }

        #[async_trait::async_trait]
        impl $crate::connector::RecommendationsProvider for $self_ty {
            async fn recommendations(
                &self,
                instrument: &$crate::Instrument,
            ) -> Result<Vec<$crate::RecommendationRow>, $crate::BorsaError> {
                let ctx = $crate::middleware::CallContext::new($crate::Capability::Recommendations);
                <Self as $crate::Middleware>::pre_call(self, &ctx).await?;
                let inner = self
                    .$inner
                    .as_recommendations_provider()
                    .ok_or_else(|| $crate::BorsaError::unsupported("recommendations"))?;
                inner
                    .recommendations(instrument)
                    .await
                    .map_err(|e| <Self as $crate::Middleware>::map_error(self, e, &ctx))
            }
        }

        #[async_trait::async_trait]
        impl $crate::connector::RecommendationsSummaryProvider for $self_ty {
            async fn recommendations_summary(
                &self,
                instrument: &$crate::Instrument,
            ) -> Result<$crate::RecommendationSummary, $crate::BorsaError> {
                let ctx = $crate::middleware::CallContext::new(
                    $crate::Capability::RecommendationsSummary,
                );
                <Self as $crate::Middleware>::pre_call(self, &ctx).await?;
                let inner = self
                    .$inner
                    .as_recommendations_summary_provider()
                    .ok_or_else(|| $crate::BorsaError::unsupported("recommendations_summary"))?;
                inner
                    .recommendations_summary(instrument)
                    .await
                    .map_err(|e| <Self as $crate::Middleware>::map_error(self, e, &ctx))
            }
        }

        #[async_trait::async_trait]
        impl $crate::connector::UpgradesDowngradesProvider for $self_ty {
            async fn upgrades_downgrades(
                &self,
                instrument: &$crate::Instrument,
            ) -> Result<Vec<$crate::UpgradeDowngradeRow>, $crate::BorsaError> {
                let ctx =
                    $crate::middleware::CallContext::new($crate::Capability::UpgradesDowngrades);
                <Self as $crate::Middleware>::pre_call(self, &ctx).await?;
                let inner = self
                    .$inner
                    .as_upgrades_downgrades_provider()
                    .ok_or_else(|| $crate::BorsaError::unsupported("upgrades_downgrades"))?;
                inner
                    .upgrades_downgrades(instrument)
                    .await
                    .map_err(|e| <Self as $crate::Middleware>::map_error(self, e, &ctx))
            }
        }

        #[async_trait::async_trait]
        impl $crate::connector::AnalystPriceTargetProvider for $self_ty {
            async fn analyst_price_target(
                &self,
                instrument: &$crate::Instrument,
            ) -> Result<$crate::PriceTarget, $crate::BorsaError> {
                let ctx =
                    $crate::middleware::CallContext::new($crate::Capability::AnalystPriceTarget);
                <Self as $crate::Middleware>::pre_call(self, &ctx).await?;
                let inner = self
                    .$inner
                    .as_analyst_price_target_provider()
                    .ok_or_else(|| $crate::BorsaError::unsupported("analyst_price_target"))?;
                inner
                    .analyst_price_target(instrument)
                    .await
                    .map_err(|e| <Self as $crate::Middleware>::map_error(self, e, &ctx))
            }
        }

        #[async_trait::async_trait]
        impl $crate::connector::MajorHoldersProvider for $self_ty {
            async fn major_holders(
                &self,
                instrument: &$crate::Instrument,
            ) -> Result<Vec<$crate::MajorHolder>, $crate::BorsaError> {
                let ctx = $crate::middleware::CallContext::new($crate::Capability::MajorHolders);
                <Self as $crate::Middleware>::pre_call(self, &ctx).await?;
                let inner = self
                    .$inner
                    .as_major_holders_provider()
                    .ok_or_else(|| $crate::BorsaError::unsupported("major_holders"))?;
                inner
                    .major_holders(instrument)
                    .await
                    .map_err(|e| <Self as $crate::Middleware>::map_error(self, e, &ctx))
            }
        }

        #[async_trait::async_trait]
        impl $crate::connector::InstitutionalHoldersProvider for $self_ty {
            async fn institutional_holders(
                &self,
                instrument: &$crate::Instrument,
            ) -> Result<Vec<$crate::InstitutionalHolder>, $crate::BorsaError> {
                let ctx =
                    $crate::middleware::CallContext::new($crate::Capability::InstitutionalHolders);
                <Self as $crate::Middleware>::pre_call(self, &ctx).await?;
                let inner = self
                    .$inner
                    .as_institutional_holders_provider()
                    .ok_or_else(|| $crate::BorsaError::unsupported("institutional_holders"))?;
                inner
                    .institutional_holders(instrument)
                    .await
                    .map_err(|e| <Self as $crate::Middleware>::map_error(self, e, &ctx))
            }
        }

        #[async_trait::async_trait]
        impl $crate::connector::MutualFundHoldersProvider for $self_ty {
            async fn mutual_fund_holders(
                &self,
                instrument: &$crate::Instrument,
            ) -> Result<Vec<$crate::InstitutionalHolder>, $crate::BorsaError> {
                let ctx =
                    $crate::middleware::CallContext::new($crate::Capability::MutualFundHolders);
                <Self as $crate::Middleware>::pre_call(self, &ctx).await?;
                let inner = self
                    .$inner
                    .as_mutual_fund_holders_provider()
                    .ok_or_else(|| $crate::BorsaError::unsupported("mutual_fund_holders"))?;
                inner
                    .mutual_fund_holders(instrument)
                    .await
                    .map_err(|e| <Self as $crate::Middleware>::map_error(self, e, &ctx))
            }
        }

        #[async_trait::async_trait]
        impl $crate::connector::InsiderTransactionsProvider for $self_ty {
            async fn insider_transactions(
                &self,
                instrument: &$crate::Instrument,
            ) -> Result<Vec<$crate::InsiderTransaction>, $crate::BorsaError> {
                let ctx =
                    $crate::middleware::CallContext::new($crate::Capability::InsiderTransactions);
                <Self as $crate::Middleware>::pre_call(self, &ctx).await?;
                let inner = self
                    .$inner
                    .as_insider_transactions_provider()
                    .ok_or_else(|| $crate::BorsaError::unsupported("insider_transactions"))?;
                inner
                    .insider_transactions(instrument)
                    .await
                    .map_err(|e| <Self as $crate::Middleware>::map_error(self, e, &ctx))
            }
        }

        #[async_trait::async_trait]
        impl $crate::connector::InsiderRosterHoldersProvider for $self_ty {
            async fn insider_roster_holders(
                &self,
                instrument: &$crate::Instrument,
            ) -> Result<Vec<$crate::InsiderRosterHolder>, $crate::BorsaError> {
                let ctx = $crate::middleware::CallContext::new($crate::Capability::InsiderRoster);
                <Self as $crate::Middleware>::pre_call(self, &ctx).await?;
                let inner = self
                    .$inner
                    .as_insider_roster_holders_provider()
                    .ok_or_else(|| $crate::BorsaError::unsupported("insider_roster_holders"))?;
                inner
                    .insider_roster_holders(instrument)
                    .await
                    .map_err(|e| <Self as $crate::Middleware>::map_error(self, e, &ctx))
            }
        }

        #[async_trait::async_trait]
        impl $crate::connector::NetSharePurchaseActivityProvider for $self_ty {
            async fn net_share_purchase_activity(
                &self,
                instrument: &$crate::Instrument,
            ) -> Result<Option<$crate::NetSharePurchaseActivity>, $crate::BorsaError> {
                let ctx = $crate::middleware::CallContext::new(
                    $crate::Capability::NetSharePurchaseActivity,
                );
                <Self as $crate::Middleware>::pre_call(self, &ctx).await?;
                let inner = self
                    .$inner
                    .as_net_share_purchase_activity_provider()
                    .ok_or_else(|| {
                        $crate::BorsaError::unsupported("net_share_purchase_activity")
                    })?;
                inner
                    .net_share_purchase_activity(instrument)
                    .await
                    .map_err(|e| <Self as $crate::Middleware>::map_error(self, e, &ctx))
            }
        }

        #[async_trait::async_trait]
        impl $crate::connector::ProfileProvider for $self_ty {
            async fn profile(
                &self,
                instrument: &$crate::Instrument,
            ) -> Result<$crate::Profile, $crate::BorsaError> {
                let ctx = $crate::middleware::CallContext::new($crate::Capability::Profile);
                <Self as $crate::Middleware>::pre_call(self, &ctx).await?;
                let inner = self
                    .$inner
                    .as_profile_provider()
                    .ok_or_else(|| $crate::BorsaError::unsupported("profile"))?;
                inner
                    .profile(instrument)
                    .await
                    .map_err(|e| <Self as $crate::Middleware>::map_error(self, e, &ctx))
            }
        }

        #[async_trait::async_trait]
        impl $crate::connector::IsinProvider for $self_ty {
            async fn isin(
                &self,
                instrument: &$crate::Instrument,
            ) -> Result<Option<$crate::Isin>, $crate::BorsaError> {
                let ctx = $crate::middleware::CallContext::new($crate::Capability::Isin);
                <Self as $crate::Middleware>::pre_call(self, &ctx).await?;
                let inner = self
                    .$inner
                    .as_isin_provider()
                    .ok_or_else(|| $crate::BorsaError::unsupported("isin"))?;
                inner
                    .isin(instrument)
                    .await
                    .map_err(|e| <Self as $crate::Middleware>::map_error(self, e, &ctx))
            }
        }

        #[async_trait::async_trait]
        impl $crate::connector::SearchProvider for $self_ty {
            async fn search(
                &self,
                req: $crate::SearchRequest,
            ) -> Result<$crate::SearchResponse, $crate::BorsaError> {
                let ctx = $crate::middleware::CallContext::new($crate::Capability::Search);
                <Self as $crate::Middleware>::pre_call(self, &ctx).await?;
                let inner = self
                    .$inner
                    .as_search_provider()
                    .ok_or_else(|| $crate::BorsaError::unsupported("search"))?;
                inner
                    .search(req)
                    .await
                    .map_err(|e| <Self as $crate::Middleware>::map_error(self, e, &ctx))
            }
        }

        #[async_trait::async_trait]
        impl $crate::connector::EsgProvider for $self_ty {
            async fn sustainability(
                &self,
                instrument: &$crate::Instrument,
            ) -> Result<$crate::EsgScores, $crate::BorsaError> {
                let ctx = $crate::middleware::CallContext::new($crate::Capability::Esg);
                <Self as $crate::Middleware>::pre_call(self, &ctx).await?;
                let inner = self
                    .$inner
                    .as_esg_provider()
                    .ok_or_else(|| $crate::BorsaError::unsupported("sustainability"))?;
                inner
                    .sustainability(instrument)
                    .await
                    .map_err(|e| <Self as $crate::Middleware>::map_error(self, e, &ctx))
            }
        }

        #[async_trait::async_trait]
        impl $crate::connector::NewsProvider for $self_ty {
            async fn news(
                &self,
                instrument: &$crate::Instrument,
                req: $crate::NewsRequest,
            ) -> Result<Vec<$crate::types::NewsArticle>, $crate::BorsaError> {
                let ctx = $crate::middleware::CallContext::new($crate::Capability::News);
                <Self as $crate::Middleware>::pre_call(self, &ctx).await?;
                let inner = self
                    .$inner
                    .as_news_provider()
                    .ok_or_else(|| $crate::BorsaError::unsupported("news"))?;
                inner
                    .news(instrument, req)
                    .await
                    .map_err(|e| <Self as $crate::Middleware>::map_error(self, e, &ctx))
            }
        }

        #[async_trait::async_trait]
        impl $crate::connector::OptionsExpirationsProvider for $self_ty {
            async fn options_expirations(
                &self,
                instrument: &$crate::Instrument,
            ) -> Result<Vec<i64>, $crate::BorsaError> {
                let ctx =
                    $crate::middleware::CallContext::new($crate::Capability::OptionsExpirations);
                <Self as $crate::Middleware>::pre_call(self, &ctx).await?;
                let inner = self
                    .$inner
                    .as_options_expirations_provider()
                    .ok_or_else(|| $crate::BorsaError::unsupported("options_expirations"))?;
                inner
                    .options_expirations(instrument)
                    .await
                    .map_err(|e| <Self as $crate::Middleware>::map_error(self, e, &ctx))
            }
        }

        #[async_trait::async_trait]
        impl $crate::connector::OptionChainProvider for $self_ty {
            async fn option_chain(
                &self,
                instrument: &$crate::Instrument,
                date: Option<i64>,
            ) -> Result<$crate::OptionChain, $crate::BorsaError> {
                let ctx = $crate::middleware::CallContext::new($crate::Capability::OptionChain);
                <Self as $crate::Middleware>::pre_call(self, &ctx).await?;
                let inner = self
                    .$inner
                    .as_option_chain_provider()
                    .ok_or_else(|| $crate::BorsaError::unsupported("option_chain"))?;
                inner
                    .option_chain(instrument, date)
                    .await
                    .map_err(|e| <Self as $crate::Middleware>::map_error(self, e, &ctx))
            }
        }

        #[async_trait::async_trait]
        impl $crate::connector::StreamProvider for $self_ty {
            async fn stream_quotes(
                &self,
                instruments: &[$crate::Instrument],
            ) -> Result<
                (
                    $crate::stream::StreamHandle,
                    tokio::sync::mpsc::Receiver<$crate::QuoteUpdate>,
                ),
                $crate::BorsaError,
            > {
                let ctx = $crate::middleware::CallContext::new($crate::Capability::StreamQuotes);
                <Self as $crate::Middleware>::pre_call(self, &ctx).await?;
                let inner = self
                    .$inner
                    .as_stream_provider()
                    .ok_or_else(|| $crate::BorsaError::unsupported("stream_quotes"))?;
                inner
                    .stream_quotes(instruments)
                    .await
                    .map_err(|e| <Self as $crate::Middleware>::map_error(self, e, &ctx))
            }
        }
    };
}
