#[cfg(feature = "test-adapters")]
use std::sync::Arc;

use async_trait::async_trait;

use borsa_core::BorsaError;
use std::time::Duration;
use yf::core::HistoryService;
use yfinance_rs as yf;

/// History abstraction (so we can inject mocks in tests).
#[async_trait]
pub trait YfHistory: Send + Sync {
    /// Fetch full history for a symbol using a provider-specific request.
    async fn fetch_full(
        &self,
        symbol: &str,
        req: yf::core::services::HistoryRequest,
    ) -> Result<yf::HistoryResponse, BorsaError>;
}

/// Quotes abstraction (so we can inject mocks in tests).
#[async_trait]
pub trait YfQuotes: Send + Sync {
    /// Fetch quotes for a batch of symbols.
    async fn fetch(&self, symbols: &[String]) -> Result<Vec<yf::core::Quote>, BorsaError>;
}

/// Search abstraction for provider-native search.
#[async_trait]
pub trait YfSearch: Send + Sync {
    /// Perform a text search and return canonical paft search results.
    async fn search(
        &self,
        req: &borsa_core::SearchRequest,
    ) -> Result<borsa_core::SearchResponse, BorsaError>;
}

/// Profile abstraction for company/fund load and optional ISIN.
#[async_trait]
pub trait YfProfile: Send + Sync {
    /// Load a company/fund profile for `symbol`.
    async fn load(&self, symbol: &str) -> Result<yf::profile::Profile, BorsaError>;

    /// Default ISIN lookup is unsupported; adapters may override.
    async fn isin(&self, _symbol: &str) -> Result<Option<String>, BorsaError> {
        Err(BorsaError::unsupported("profile/isin"))
    }
}

/// Fundamentals abstraction for earnings and financial statements.
///
/// Default methods return `unsupported` so tests can override only the endpoints they need.
#[async_trait]
pub trait YfFundamentals: Send + Sync {
    /// Fetch earnings for `symbol`.
    async fn earnings(&self, symbol: &str) -> Result<yf::fundamentals::Earnings, BorsaError>;

    /// Fetch income statement rows. Default returns `unsupported`.
    async fn income_statement(
        &self,
        _symbol: &str,
        _quarterly: bool,
    ) -> Result<Vec<yf::fundamentals::IncomeStatementRow>, BorsaError> {
        Err(BorsaError::unsupported("fundamentals/income_statement"))
    }

    /// Fetch balance sheet rows. Default returns `unsupported`.
    async fn balance_sheet(
        &self,
        _symbol: &str,
        _quarterly: bool,
    ) -> Result<Vec<yf::fundamentals::BalanceSheetRow>, BorsaError> {
        Err(BorsaError::unsupported("fundamentals/balance_sheet"))
    }

    /// Fetch cashflow rows. Default returns `unsupported`.
    async fn cashflow(
        &self,
        _symbol: &str,
        _quarterly: bool,
    ) -> Result<Vec<yf::fundamentals::CashflowRow>, BorsaError> {
        Err(BorsaError::unsupported("fundamentals/cashflow"))
    }

    /// Fetch earnings/IPO/dividend calendar. Default returns `unsupported`.
    async fn calendar(&self, _symbol: &str) -> Result<yf::fundamentals::Calendar, BorsaError> {
        Err(BorsaError::unsupported("fundamentals/calendar"))
    }
}

/// Options abstraction for expirations and option chain.
#[async_trait]
pub trait YfOptions: Send + Sync {
    /// Fetch available option expiration dates.
    async fn expirations(&self, symbol: &str) -> Result<Vec<i64>, BorsaError>;
    /// Fetch the option chain for an optional expiration.
    async fn chain(
        &self,
        symbol: &str,
        date: Option<i64>,
    ) -> Result<yf::ticker::OptionChain, BorsaError>;
}

/// Analyst analysis abstraction including recommendations and targets.
#[async_trait]
pub trait YfAnalysis: Send + Sync {
    /// Fetch recommendation rows.
    async fn recommendations(
        &self,
        symbol: &str,
    ) -> Result<Vec<yf::analysis::RecommendationRow>, BorsaError>;
    /// Fetch recommendations summary.
    async fn recommendations_summary(
        &self,
        symbol: &str,
    ) -> Result<yf::analysis::RecommendationSummary, BorsaError>;
    /// Fetch broker upgrades/downgrades.
    async fn upgrades_downgrades(
        &self,
        symbol: &str,
    ) -> Result<Vec<yf::analysis::UpgradeDowngradeRow>, BorsaError>;
    /// Fetch analyst price target.
    async fn analyst_price_target(
        &self,
        symbol: &str,
    ) -> Result<yf::analysis::PriceTarget, BorsaError>;
}

/// Holders abstraction for major/institutional/mutual and insider activity.
#[async_trait]
pub trait YfHolders: Send + Sync {
    /// Fetch major holders summary rows.
    async fn major_holders(
        &self,
        symbol: &str,
    ) -> Result<Vec<yf::holders::MajorHolder>, BorsaError>;
    /// Fetch institutional holders.
    async fn institutional_holders(
        &self,
        symbol: &str,
    ) -> Result<Vec<yf::holders::InstitutionalHolder>, BorsaError>;
    /// Fetch mutual fund holders.
    async fn mutual_fund_holders(
        &self,
        symbol: &str,
    ) -> Result<Vec<yf::holders::InstitutionalHolder>, BorsaError>;
    /// Fetch insider transactions.
    async fn insider_transactions(
        &self,
        symbol: &str,
    ) -> Result<Vec<yf::holders::InsiderTransaction>, BorsaError>;
    /// Fetch insider roster holders.
    async fn insider_roster_holders(
        &self,
        symbol: &str,
    ) -> Result<Vec<yf::holders::InsiderRosterHolder>, BorsaError>;
    /// Fetch net share purchase activity.
    async fn net_share_purchase_activity(
        &self,
        symbol: &str,
    ) -> Result<Option<yf::holders::NetSharePurchaseActivity>, BorsaError>;
}

/// ESG abstraction for sustainability scores.
#[async_trait]
pub trait YfEsg: Send + Sync {
    /// Fetch ESG scores for `symbol`.
    async fn sustainability(&self, symbol: &str) -> Result<yf::esg::EsgScores, BorsaError>;
}

/// News abstraction for fetching articles.
#[async_trait]
pub trait YfNews: Send + Sync {
    /// Fetch news articles for a symbol.
    async fn news(
        &self,
        symbol: &str,
        req: borsa_core::NewsRequest,
    ) -> Result<Vec<yf::news::NewsArticle>, BorsaError>;
}

/// Streaming abstraction for quote updates.
#[async_trait]
pub trait YfStream: Send + Sync {
    /// Start streaming quote updates for the given symbols.
    async fn start(
        &self,
        symbols: &[String],
    ) -> Result<
        (
            borsa_core::stream::StreamHandle,
            tokio::sync::mpsc::Receiver<borsa_core::QuoteUpdate>,
        ),
        BorsaError,
    >;
}

/// Real adapter backed by a single `YfClient` instance.
/// `YfClient` is `Clone + Send + Sync`, so no external locking is needed.
#[derive(Clone)]
pub struct RealAdapter {
    client: yf::YfClient,
}

impl RealAdapter {
    /// Build a default `YfClient` with a recommended user agent.
    ///
    /// # Panics
    /// Panics if building the underlying `YfClient` fails, which is unexpected
    /// in normal environments (invalid user agent configuration).
    #[must_use]
    pub fn new_default() -> Self {
        let http = reqwest::Client::builder()
            .cookie_store(true)
            .no_proxy()
            .build()
            .expect("Failed to build reqwest client for YfClient");
        Self {
            client: yf::YfClient::builder()
                .custom_client(http)
                .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
                .build()
                .expect("Failed to build YfClient with user agent"),
        }
    }
    /// Wrap an existing `YfClient`.
    #[must_use]
    pub const fn new(client: yf::YfClient) -> Self {
        Self { client }
    }
}

fn map_yf_err(e: &yf::YfError, context: &str) -> BorsaError {
    match e {
        yf::YfError::NotFound { .. } => BorsaError::not_found(context.to_string()),
        yf::YfError::RateLimited { .. } => {
            BorsaError::connector("borsa-yfinance", format!("rate limit: {context}"))
        }
        yf::YfError::ServerError { status, .. } => BorsaError::connector(
            "borsa-yfinance",
            format!("server error {status}: {context}"),
        ),
        yf::YfError::Status { status, .. } => {
            BorsaError::connector("borsa-yfinance", format!("status {status}: {context}"))
        }
        other => BorsaError::connector("borsa-yfinance", other.to_string()),
    }
}

#[async_trait]
impl YfHistory for RealAdapter {
    async fn fetch_full(
        &self,
        symbol: &str,
        req: yf::core::services::HistoryRequest,
    ) -> Result<yf::HistoryResponse, BorsaError> {
        // `YfClient` implements `HistoryService`, which we use directly.
        self.client
            .fetch_full_history(symbol, req)
            .await
            .map_err(|e| map_yf_err(&e, &format!("history for {symbol}")))
    }
}

#[async_trait]
impl YfQuotes for RealAdapter {
    async fn fetch(&self, symbols: &[String]) -> Result<Vec<yf::core::Quote>, BorsaError> {
        let quotes = yf::quote::quotes(&self.client, symbols.iter().cloned())
            .await
            .map_err(|e| map_yf_err(&e, "quotes"))?;
        Ok(quotes)
    }
}

#[async_trait]
impl YfStream for RealAdapter {
    async fn start(
        &self,
        symbols: &[String],
    ) -> Result<
        (
            borsa_core::stream::StreamHandle,
            tokio::sync::mpsc::Receiver<borsa_core::QuoteUpdate>,
        ),
        BorsaError,
    > {
        let builder = yf::stream::StreamBuilder::new(&self.client)
            .method(yf::stream::StreamMethod::WebsocketWithFallback)
            .interval(Duration::from_secs(1))
            .symbols(symbols.iter().cloned());
        let (handle, rx) = builder.start().map_err(|e| map_yf_err(&e, "stream"))?;

        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
        let join = tokio::spawn(async move {
            // Propagate stop signal to the underlying yfinance stream handle.
            let _ = stop_rx.await;
            handle.stop().await;
        });

        Ok((borsa_core::stream::StreamHandle::new(join, stop_tx), rx))
    }
}

#[async_trait]
impl YfSearch for RealAdapter {
    async fn search(
        &self,
        req: &borsa_core::SearchRequest,
    ) -> Result<borsa_core::SearchResponse, BorsaError> {
        let mut builder = yf::search::SearchBuilder::new(&self.client, req.query());

        if let Some(limit) = req.limit() {
            builder = builder.quotes_count(
                u32::try_from(limit)
                    .map_err(|_| BorsaError::InvalidArg("limit too large for provider".into()))?,
            );
        }

        let resp = builder
            .fetch()
            .await
            .map_err(|e| map_yf_err(&e, "search"))?;

        Ok(resp)
    }
}

#[async_trait]
impl YfProfile for RealAdapter {
    async fn load(&self, symbol: &str) -> Result<yf::profile::Profile, BorsaError> {
        yf::profile::load_profile(&self.client, symbol)
            .await
            .map_err(|e| map_yf_err(&e, &format!("profile for {symbol}")))
    }

    async fn isin(&self, symbol: &str) -> Result<Option<String>, BorsaError> {
        yfinance_rs::ticker::Ticker::new(&self.client, symbol.to_string())
            .isin()
            .await
            .map_err(|e| map_yf_err(&e, &format!("isin for {symbol}")))
    }
}

#[async_trait]
impl YfFundamentals for RealAdapter {
    async fn earnings(&self, symbol: &str) -> Result<yf::fundamentals::Earnings, BorsaError> {
        let fb = yf::fundamentals::FundamentalsBuilder::new(&self.client, symbol.to_string());
        fb.earnings(None)
            .await
            .map_err(|e| map_yf_err(&e, &format!("earnings for {symbol}")))
    }

    async fn income_statement(
        &self,
        symbol: &str,
        quarterly: bool,
    ) -> Result<Vec<yf::fundamentals::IncomeStatementRow>, BorsaError> {
        let fb = yf::fundamentals::FundamentalsBuilder::new(&self.client, symbol.to_string());
        fb.income_statement(quarterly, None)
            .await
            .map_err(|e| map_yf_err(&e, &format!("income statement for {symbol}")))
    }

    async fn balance_sheet(
        &self,
        symbol: &str,
        quarterly: bool,
    ) -> Result<Vec<yf::fundamentals::BalanceSheetRow>, BorsaError> {
        let fb = yf::fundamentals::FundamentalsBuilder::new(&self.client, symbol.to_string());
        fb.balance_sheet(quarterly, None)
            .await
            .map_err(|e| map_yf_err(&e, &format!("balance sheet for {symbol}")))
    }

    async fn cashflow(
        &self,
        symbol: &str,
        quarterly: bool,
    ) -> Result<Vec<yf::fundamentals::CashflowRow>, BorsaError> {
        let fb = yf::fundamentals::FundamentalsBuilder::new(&self.client, symbol.to_string());
        fb.cashflow(quarterly, None)
            .await
            .map_err(|e| map_yf_err(&e, &format!("cashflow for {symbol}")))
    }

    async fn calendar(&self, symbol: &str) -> Result<yf::fundamentals::Calendar, BorsaError> {
        let fb = yf::fundamentals::FundamentalsBuilder::new(&self.client, symbol.to_string());
        fb.calendar()
            .await
            .map_err(|e| map_yf_err(&e, &format!("calendar for {symbol}")))
    }
}

#[async_trait]
impl YfOptions for RealAdapter {
    async fn expirations(&self, symbol: &str) -> Result<Vec<i64>, BorsaError> {
        let t = yf::ticker::Ticker::new(&self.client, symbol.to_string());
        t.options()
            .await
            .map_err(|e| map_yf_err(&e, &format!("options expirations for {symbol}")))
    }

    async fn chain(
        &self,
        symbol: &str,
        date: Option<i64>,
    ) -> Result<yf::ticker::OptionChain, BorsaError> {
        let t = yf::ticker::Ticker::new(&self.client, symbol.to_string());
        t.option_chain(date)
            .await
            .map_err(|e| map_yf_err(&e, &format!("option chain for {symbol}")))
    }
}

#[async_trait]
impl YfAnalysis for RealAdapter {
    async fn recommendations(
        &self,
        symbol: &str,
    ) -> Result<Vec<yf::analysis::RecommendationRow>, BorsaError> {
        let ab = yf::analysis::AnalysisBuilder::new(&self.client, symbol.to_string());
        ab.recommendations()
            .await
            .map_err(|e| map_yf_err(&e, &format!("recommendations for {symbol}")))
    }

    async fn recommendations_summary(
        &self,
        symbol: &str,
    ) -> Result<yf::analysis::RecommendationSummary, BorsaError> {
        let ab = yf::analysis::AnalysisBuilder::new(&self.client, symbol.to_string());
        ab.recommendations_summary()
            .await
            .map_err(|e| map_yf_err(&e, &format!("recommendations summary for {symbol}")))
    }

    async fn upgrades_downgrades(
        &self,
        symbol: &str,
    ) -> Result<Vec<yf::analysis::UpgradeDowngradeRow>, BorsaError> {
        let ab = yf::analysis::AnalysisBuilder::new(&self.client, symbol.to_string());
        ab.upgrades_downgrades()
            .await
            .map_err(|e| map_yf_err(&e, &format!("upgrades downgrades for {symbol}")))
    }

    async fn analyst_price_target(
        &self,
        symbol: &str,
    ) -> Result<yf::analysis::PriceTarget, BorsaError> {
        let ab = yf::analysis::AnalysisBuilder::new(&self.client, symbol.to_string());
        ab.analyst_price_target(None)
            .await
            .map_err(|e| map_yf_err(&e, &format!("analyst price target for {symbol}")))
    }
}

#[async_trait]
impl YfHolders for RealAdapter {
    async fn major_holders(
        &self,
        symbol: &str,
    ) -> Result<Vec<yf::holders::MajorHolder>, BorsaError> {
        let hb = yf::holders::HoldersBuilder::new(&self.client, symbol.to_string());
        hb.major_holders()
            .await
            .map_err(|e| map_yf_err(&e, &format!("major holders for {symbol}")))
    }

    async fn institutional_holders(
        &self,
        symbol: &str,
    ) -> Result<Vec<yf::holders::InstitutionalHolder>, BorsaError> {
        let hb = yf::holders::HoldersBuilder::new(&self.client, symbol.to_string());
        hb.institutional_holders()
            .await
            .map_err(|e| map_yf_err(&e, &format!("institutional holders for {symbol}")))
    }

    async fn mutual_fund_holders(
        &self,
        symbol: &str,
    ) -> Result<Vec<yf::holders::InstitutionalHolder>, BorsaError> {
        let hb = yf::holders::HoldersBuilder::new(&self.client, symbol.to_string());
        hb.mutual_fund_holders()
            .await
            .map_err(|e| map_yf_err(&e, &format!("mutual fund holders for {symbol}")))
    }

    async fn insider_transactions(
        &self,
        symbol: &str,
    ) -> Result<Vec<yf::holders::InsiderTransaction>, BorsaError> {
        let hb = yf::holders::HoldersBuilder::new(&self.client, symbol.to_string());
        hb.insider_transactions()
            .await
            .map_err(|e| map_yf_err(&e, &format!("insider transactions for {symbol}")))
    }

    async fn insider_roster_holders(
        &self,
        symbol: &str,
    ) -> Result<Vec<yf::holders::InsiderRosterHolder>, BorsaError> {
        let hb = yf::holders::HoldersBuilder::new(&self.client, symbol.to_string());
        hb.insider_roster_holders()
            .await
            .map_err(|e| map_yf_err(&e, &format!("insider roster holders for {symbol}")))
    }

    async fn net_share_purchase_activity(
        &self,
        symbol: &str,
    ) -> Result<Option<yf::holders::NetSharePurchaseActivity>, BorsaError> {
        let hb = yf::holders::HoldersBuilder::new(&self.client, symbol.to_string());
        hb.net_share_purchase_activity()
            .await
            .map_err(|e| map_yf_err(&e, &format!("net share purchase activity for {symbol}")))
    }
}

#[async_trait]
impl YfEsg for RealAdapter {
    async fn sustainability(&self, symbol: &str) -> Result<yf::esg::EsgScores, BorsaError> {
        let eb = yf::esg::EsgBuilder::new(&self.client, symbol);
        let summary = eb
            .fetch()
            .await
            .map_err(|e| map_yf_err(&e, &format!("sustainability for {symbol}")))?;
        summary
            .scores
            .map_or_else(|| Err(BorsaError::Data("missing ESG scores".into())), Ok)
    }
}

#[async_trait]
impl YfNews for RealAdapter {
    async fn news(
        &self,
        symbol: &str,
        req: borsa_core::NewsRequest,
    ) -> Result<Vec<yf::news::NewsArticle>, BorsaError> {
        let nb = yf::news::NewsBuilder::new(&self.client, symbol)
            .count(req.count)
            .tab(match req.tab {
                borsa_core::NewsTab::News => yf::news::NewsTab::News,
                borsa_core::NewsTab::All => yf::news::NewsTab::All,
                borsa_core::NewsTab::PressReleases => yf::news::NewsTab::PressReleases,
            });
        nb.fetch()
            .await
            .map_err(|e| map_yf_err(&e, &format!("news for {symbol}")))
    }
}

/* -------- Test-only lightweight adapter constructors ------- */

#[cfg(feature = "test-adapters")]
impl dyn YfHistory {
    /// Build a `YfHistory` from a closure (tests only).
    pub fn from_fn<F>(f: F) -> Arc<dyn YfHistory>
    where
        F: Send
            + Sync
            + 'static
            + Fn(
                String,
                yf::core::services::HistoryRequest,
            ) -> Result<yf::HistoryResponse, BorsaError>,
    {
        struct FnHist<F>(F);
        #[async_trait]
        impl<F> YfHistory for FnHist<F>
        where
            F: Send
                + Sync
                + 'static
                + Fn(
                    String,
                    yf::core::services::HistoryRequest,
                ) -> Result<yf::HistoryResponse, BorsaError>,
        {
            async fn fetch_full(
                &self,
                symbol: &str,
                req: yf::core::services::HistoryRequest,
            ) -> Result<yf::HistoryResponse, BorsaError> {
                (self.0)(symbol.to_string(), req)
            }
        }
        Arc::new(FnHist(f))
    }
}
#[cfg(feature = "test-adapters")]
impl dyn YfQuotes {
    /// Build a `YfQuotes` from a closure (tests only).
    pub fn from_fn<F>(f: F) -> Arc<dyn YfQuotes>
    where
        F: Send + Sync + 'static + Fn(Vec<String>) -> Result<Vec<yf::core::Quote>, BorsaError>,
    {
        struct FnQuotes<F>(F);
        #[async_trait]
        impl<F> YfQuotes for FnQuotes<F>
        where
            F: Send + Sync + 'static + Fn(Vec<String>) -> Result<Vec<yf::core::Quote>, BorsaError>,
        {
            async fn fetch(&self, symbols: &[String]) -> Result<Vec<yf::core::Quote>, BorsaError> {
                (self.0)(symbols.to_vec())
            }
        }
        Arc::new(FnQuotes(f))
    }
}

#[cfg(feature = "test-adapters")]
impl dyn YfSearch {
    /// Test helper that builds a `YfSearch` from a closure taking the raw query `&str`
    /// and returning a list of symbol strings. Symbols are mapped to minimal `yf::SearchQuote`s.
    pub fn from_fn<F>(f: F) -> Arc<dyn YfSearch>
    where
        F: Send + Sync + 'static + Fn(&str) -> Result<Vec<String>, BorsaError>,
    {
        struct FnSearch<F>(F);

        #[async_trait]
        impl<F> YfSearch for FnSearch<F>
        where
            F: Send + Sync + 'static + Fn(&str) -> Result<Vec<String>, BorsaError>,
        {
            async fn search(
                &self,
                req: &borsa_core::SearchRequest,
            ) -> Result<borsa_core::SearchResponse, BorsaError> {
                let symbols = (self.0)(req.query())?;
                let mut results = Vec::with_capacity(symbols.len());
                for symbol in symbols {
                    let sym = borsa_core::Symbol::new(&symbol)
                        .map_err(|e| BorsaError::InvalidArg(e.to_string()))?;
                    results.push(borsa_core::SearchResult {
                        symbol: sym,
                        name: None,
                        exchange: None,
                        kind: borsa_core::AssetKind::Equity,
                    });
                }
                Ok(borsa_core::SearchResponse { results })
            }
        }

        Arc::new(FnSearch(f))
    }
}

#[cfg(feature = "test-adapters")]
impl dyn YfProfile {
    /// Build a `YfProfile` from closures (tests only).
    pub fn from_fns<FLoad, FIsin>(fload: FLoad, fisin: FIsin) -> Arc<dyn YfProfile>
    where
        FLoad: Send + Sync + 'static + Fn(String) -> Result<yf::profile::Profile, BorsaError>,
        FIsin: Send + Sync + 'static + Fn(String) -> Result<Option<String>, BorsaError>,
    {
        struct FnProfile<FLoad, FIsin> {
            fload: FLoad,
            fisin: FIsin,
        }
        #[async_trait]
        impl<FLoad, FIsin> YfProfile for FnProfile<FLoad, FIsin>
        where
            FLoad: Send + Sync + 'static + Fn(String) -> Result<yf::profile::Profile, BorsaError>,
            FIsin: Send + Sync + 'static + Fn(String) -> Result<Option<String>, BorsaError>,
        {
            async fn load(&self, symbol: &str) -> Result<yf::profile::Profile, BorsaError> {
                (self.fload)(symbol.to_string())
            }
            async fn isin(&self, symbol: &str) -> Result<Option<String>, BorsaError> {
                (self.fisin)(symbol.to_string())
            }
        }
        Arc::new(FnProfile { fload, fisin })
    }

    /// Build a `YfProfile` from a single closure (tests only).
    pub fn from_fn<F>(f: F) -> Arc<dyn YfProfile>
    where
        F: Send + Sync + 'static + Fn(String) -> Result<yf::profile::Profile, BorsaError>,
    {
        Self::from_fns(f, |_| Err(BorsaError::unsupported("profile/isin")))
    }
}

// In `borsa-yfinance/src/adapter.rs`, update `impl dyn YfFundamentals { pub fn from_fns(...) ... }`:
#[cfg(feature = "test-adapters")]
impl dyn YfFundamentals {
    /// Build a `YfFundamentals` from closures (tests only).
    pub fn from_fns<FI, FB, FC, FCal, FEarn>(
        fi: FI,
        fb: FB,
        fc: FC,
        fcal: FCal,
        fearn: FEarn,
    ) -> Arc<dyn YfFundamentals>
    where
        FI: Send
            + Sync
            + 'static
            + Fn(String, bool) -> Result<Vec<yf::fundamentals::IncomeStatementRow>, BorsaError>,
        FB: Send
            + Sync
            + 'static
            + Fn(String, bool) -> Result<Vec<yf::fundamentals::BalanceSheetRow>, BorsaError>,
        FC: Send
            + Sync
            + 'static
            + Fn(String, bool) -> Result<Vec<yf::fundamentals::CashflowRow>, BorsaError>,
        FCal: Send + Sync + 'static + Fn(String) -> Result<yf::fundamentals::Calendar, BorsaError>,
        FEarn: Send + Sync + 'static + Fn(String) -> Result<yf::fundamentals::Earnings, BorsaError>,
    {
        struct FnFundamentals<FI, FB, FC, FCal, FEarn> {
            fi: FI,
            fb: FB,
            fc: FC,
            fcal: FCal,
            fearn: FEarn,
        }

        #[async_trait]
        impl<FI, FB, FC, FCal, FEarn> YfFundamentals for FnFundamentals<FI, FB, FC, FCal, FEarn>
        where
            FI: Send
                + Sync
                + 'static
                + Fn(String, bool) -> Result<Vec<yf::fundamentals::IncomeStatementRow>, BorsaError>,
            FB: Send
                + Sync
                + 'static
                + Fn(String, bool) -> Result<Vec<yf::fundamentals::BalanceSheetRow>, BorsaError>,
            FC: Send
                + Sync
                + 'static
                + Fn(String, bool) -> Result<Vec<yf::fundamentals::CashflowRow>, BorsaError>,
            FCal: Send
                + Sync
                + 'static
                + Fn(String) -> Result<yf::fundamentals::Calendar, BorsaError>,
            FEarn: Send
                + Sync
                + 'static
                + Fn(String) -> Result<yf::fundamentals::Earnings, BorsaError>,
        {
            async fn income_statement(
                &self,
                symbol: &str,
                quarterly: bool,
            ) -> Result<Vec<yf::fundamentals::IncomeStatementRow>, BorsaError> {
                (self.fi)(symbol.to_string(), quarterly)
            }

            async fn balance_sheet(
                &self,
                symbol: &str,
                quarterly: bool,
            ) -> Result<Vec<yf::fundamentals::BalanceSheetRow>, BorsaError> {
                (self.fb)(symbol.to_string(), quarterly)
            }

            async fn cashflow(
                &self,
                symbol: &str,
                quarterly: bool,
            ) -> Result<Vec<yf::fundamentals::CashflowRow>, BorsaError> {
                (self.fc)(symbol.to_string(), quarterly)
            }

            async fn calendar(
                &self,
                symbol: &str,
            ) -> Result<yf::fundamentals::Calendar, BorsaError> {
                (self.fcal)(symbol.to_string())
            }

            async fn earnings(
                &self,
                symbol: &str,
            ) -> Result<yf::fundamentals::Earnings, BorsaError> {
                (self.fearn)(symbol.to_string())
            }
        }

        Arc::new(FnFundamentals {
            fi,
            fb,
            fc,
            fcal,
            fearn,
        })
    }
}

#[cfg(feature = "test-adapters")]
impl dyn YfOptions {
    /// Build a `YfOptions` from closures (tests only).
    pub fn from_fns<FE, FC>(fe: FE, fc: FC) -> Arc<dyn YfOptions>
    where
        FE: Send + Sync + 'static + Fn(String) -> Result<Vec<i64>, BorsaError>,
        FC: Send
            + Sync
            + 'static
            + Fn(String, Option<i64>) -> Result<yf::ticker::OptionChain, BorsaError>,
    {
        struct FnOptions<FE, FC> {
            fe: FE,
            fc: FC,
        }
        #[async_trait]
        impl<FE, FC> YfOptions for FnOptions<FE, FC>
        where
            FE: Send + Sync + 'static + Fn(String) -> Result<Vec<i64>, BorsaError>,
            FC: Send
                + Sync
                + 'static
                + Fn(String, Option<i64>) -> Result<yf::ticker::OptionChain, BorsaError>,
        {
            async fn expirations(&self, symbol: &str) -> Result<Vec<i64>, BorsaError> {
                (self.fe)(symbol.to_string())
            }
            async fn chain(
                &self,
                symbol: &str,
                date: Option<i64>,
            ) -> Result<yf::ticker::OptionChain, BorsaError> {
                (self.fc)(symbol.to_string(), date)
            }
        }
        Arc::new(FnOptions { fe, fc })
    }
}

#[cfg(feature = "test-adapters")]
impl dyn YfAnalysis {
    /// Build a `YfAnalysis` from closures (tests only).
    pub fn from_fns<FR, FRS, FUD, FPT>(fr: FR, frs: FRS, fud: FUD, fpt: FPT) -> Arc<dyn YfAnalysis>
    where
        FR: Send
            + Sync
            + 'static
            + Fn(String) -> Result<Vec<yf::analysis::RecommendationRow>, BorsaError>,
        FRS: Send
            + Sync
            + 'static
            + Fn(String) -> Result<yf::analysis::RecommendationSummary, BorsaError>,
        FUD: Send
            + Sync
            + 'static
            + Fn(String) -> Result<Vec<yf::analysis::UpgradeDowngradeRow>, BorsaError>,
        FPT: Send + Sync + 'static + Fn(String) -> Result<yf::analysis::PriceTarget, BorsaError>,
    {
        struct FnAnalysis<FR, FRS, FUD, FPT> {
            fr: FR,
            frs: FRS,
            fud: FUD,
            fpt: FPT,
        }
        #[async_trait]
        impl<FR, FRS, FUD, FPT> YfAnalysis for FnAnalysis<FR, FRS, FUD, FPT>
        where
            FR: Send
                + Sync
                + 'static
                + Fn(String) -> Result<Vec<yf::analysis::RecommendationRow>, BorsaError>,
            FRS: Send
                + Sync
                + 'static
                + Fn(String) -> Result<yf::analysis::RecommendationSummary, BorsaError>,
            FUD: Send
                + Sync
                + 'static
                + Fn(String) -> Result<Vec<yf::analysis::UpgradeDowngradeRow>, BorsaError>,
            FPT:
                Send + Sync + 'static + Fn(String) -> Result<yf::analysis::PriceTarget, BorsaError>,
        {
            async fn recommendations(
                &self,
                s: &str,
            ) -> Result<Vec<yf::analysis::RecommendationRow>, BorsaError> {
                (self.fr)(s.to_string())
            }
            async fn recommendations_summary(
                &self,
                s: &str,
            ) -> Result<yf::analysis::RecommendationSummary, BorsaError> {
                (self.frs)(s.to_string())
            }
            async fn upgrades_downgrades(
                &self,
                s: &str,
            ) -> Result<Vec<yf::analysis::UpgradeDowngradeRow>, BorsaError> {
                (self.fud)(s.to_string())
            }
            async fn analyst_price_target(
                &self,
                s: &str,
            ) -> Result<yf::analysis::PriceTarget, BorsaError> {
                (self.fpt)(s.to_string())
            }
        }
        Arc::new(FnAnalysis { fr, frs, fud, fpt })
    }
}

#[cfg(feature = "test-adapters")]
impl dyn YfHolders {
    /// Build a `YfHolders` from closures (tests only).
    pub fn from_fns<FMaj, FInst, FMut, FTrans, FRoster, FNet>(
        fmaj: FMaj,
        finst: FInst,
        fmut: FMut,
        ftrans: FTrans,
        froster: FRoster,
        fnet: FNet,
    ) -> Arc<dyn YfHolders>
    where
        FMaj:
            Send + Sync + 'static + Fn(String) -> Result<Vec<yf::holders::MajorHolder>, BorsaError>,
        FInst: Send
            + Sync
            + 'static
            + Fn(String) -> Result<Vec<yf::holders::InstitutionalHolder>, BorsaError>,
        FMut: Send
            + Sync
            + 'static
            + Fn(String) -> Result<Vec<yf::holders::InstitutionalHolder>, BorsaError>,
        FTrans: Send
            + Sync
            + 'static
            + Fn(String) -> Result<Vec<yf::holders::InsiderTransaction>, BorsaError>,
        FRoster: Send
            + Sync
            + 'static
            + Fn(String) -> Result<Vec<yf::holders::InsiderRosterHolder>, BorsaError>,
        FNet: Send
            + Sync
            + 'static
            + Fn(String) -> Result<Option<yf::holders::NetSharePurchaseActivity>, BorsaError>,
    {
        struct FnHander<FMaj, FInst, FMut, FTrans, FRoster, FNet> {
            fmaj: FMaj,
            finst: FInst,
            fmut: FMut,
            ftrans: FTrans,
            froster: FRoster,
            fnet: FNet,
        }

        #[async_trait]
        impl<FMaj, FInst, FMut, FTrans, FRoster, FNet> YfHolders
            for FnHander<FMaj, FInst, FMut, FTrans, FRoster, FNet>
        where
            FMaj: Send
                + Sync
                + 'static
                + Fn(String) -> Result<Vec<yf::holders::MajorHolder>, BorsaError>,
            FInst: Send
                + Sync
                + 'static
                + Fn(String) -> Result<Vec<yf::holders::InstitutionalHolder>, BorsaError>,
            FMut: Send
                + Sync
                + 'static
                + Fn(String) -> Result<Vec<yf::holders::InstitutionalHolder>, BorsaError>,
            FTrans: Send
                + Sync
                + 'static
                + Fn(String) -> Result<Vec<yf::holders::InsiderTransaction>, BorsaError>,
            FRoster: Send
                + Sync
                + 'static
                + Fn(String) -> Result<Vec<yf::holders::InsiderRosterHolder>, BorsaError>,
            FNet: Send
                + Sync
                + 'static
                + Fn(String) -> Result<Option<yf::holders::NetSharePurchaseActivity>, BorsaError>,
        {
            async fn major_holders(
                &self,
                s: &str,
            ) -> Result<Vec<yf::holders::MajorHolder>, BorsaError> {
                (self.fmaj)(s.to_string())
            }
            async fn institutional_holders(
                &self,
                s: &str,
            ) -> Result<Vec<yf::holders::InstitutionalHolder>, BorsaError> {
                (self.finst)(s.to_string())
            }
            async fn mutual_fund_holders(
                &self,
                s: &str,
            ) -> Result<Vec<yf::holders::InstitutionalHolder>, BorsaError> {
                (self.fmut)(s.to_string())
            }
            async fn insider_transactions(
                &self,
                s: &str,
            ) -> Result<Vec<yf::holders::InsiderTransaction>, BorsaError> {
                (self.ftrans)(s.to_string())
            }
            async fn insider_roster_holders(
                &self,
                s: &str,
            ) -> Result<Vec<yf::holders::InsiderRosterHolder>, BorsaError> {
                (self.froster)(s.to_string())
            }
            async fn net_share_purchase_activity(
                &self,
                s: &str,
            ) -> Result<Option<yf::holders::NetSharePurchaseActivity>, BorsaError> {
                (self.fnet)(s.to_string())
            }
        }
        Arc::new(FnHander {
            fmaj,
            finst,
            fmut,
            ftrans,
            froster,
            fnet,
        })
    }
}

#[cfg(feature = "test-adapters")]
impl dyn YfEsg {
    /// Build a `YfEsg` from a closure (tests only).
    pub fn from_fn<F>(f: F) -> Arc<dyn YfEsg>
    where
        F: Send + Sync + 'static + Fn(String) -> Result<yf::esg::EsgScores, BorsaError>,
    {
        struct FnEsg<F>(F);
        #[async_trait]
        impl<F> YfEsg for FnEsg<F>
        where
            F: Send + Sync + 'static + Fn(String) -> Result<yf::esg::EsgScores, BorsaError>,
        {
            async fn sustainability(&self, symbol: &str) -> Result<yf::esg::EsgScores, BorsaError> {
                (self.0)(symbol.to_string())
            }
        }
        Arc::new(FnEsg(f))
    }
}

#[cfg(feature = "test-adapters")]
impl dyn YfNews {
    /// Build a `YfNews` from a closure (tests only).
    pub fn from_fn<F>(f: F) -> Arc<dyn YfNews>
    where
        F: Send
            + Sync
            + 'static
            + Fn(String, borsa_core::NewsRequest) -> Result<Vec<yf::news::NewsArticle>, BorsaError>,
    {
        struct FnNews<F>(F);
        #[async_trait]
        impl<F> YfNews for FnNews<F>
        where
            F: Send
                + Sync
                + 'static
                + Fn(
                    String,
                    borsa_core::NewsRequest,
                ) -> Result<Vec<yf::news::NewsArticle>, BorsaError>,
        {
            async fn news(
                &self,
                symbol: &str,
                req: borsa_core::NewsRequest,
            ) -> Result<Vec<yf::news::NewsArticle>, BorsaError> {
                (self.0)(symbol.to_string(), req)
            }
        }
        Arc::new(FnNews(f))
    }
}

// Convenience so connector can take a single adapter and split it into both trait objects.
/// Helper trait to split a concrete adapter into arc trait objects.
#[cfg(feature = "test-adapters")]
pub trait CloneArcAdapters {
    /// Clone as `Arc<dyn YfHistory>`.
    fn clone_arc_history(&self) -> Arc<dyn YfHistory> {
        <dyn YfHistory>::from_fn(|_, _| Err(BorsaError::unsupported("history")))
    }
    /// Clone as `Arc<dyn YfQuotes>`.
    fn clone_arc_quotes(&self) -> Arc<dyn YfQuotes> {
        <dyn YfQuotes>::from_fn(|_| Err(BorsaError::unsupported("quote")))
    }
    /// Clone as `Arc<dyn YfSearch>`.
    fn clone_arc_search(&self) -> Arc<dyn YfSearch> {
        <dyn YfSearch>::from_fn(|_| Err(BorsaError::unsupported("search")))
    }
    /// Clone as `Arc<dyn YfProfile>`.
    fn clone_arc_profile(&self) -> Arc<dyn YfProfile> {
        <dyn YfProfile>::from_fns(
            |_| Err(BorsaError::unsupported("profile")),
            |_| Err(BorsaError::unsupported("profile/isin")),
        )
    }
    /// Clone as `Arc<dyn YfFundamentals>`.
    fn clone_arc_fundamentals(&self) -> std::sync::Arc<dyn YfFundamentals> {
        <dyn YfFundamentals>::from_fns(
            |_s, _q| {
                Err(borsa_core::BorsaError::unsupported(
                    "fundamentals/income_statement",
                ))
            },
            |_s, _q| {
                Err(borsa_core::BorsaError::unsupported(
                    "fundamentals/balance_sheet",
                ))
            },
            |_s, _q| Err(borsa_core::BorsaError::unsupported("fundamentals/cashflow")),
            |_s| Err(borsa_core::BorsaError::unsupported("fundamentals/calendar")),
            |_s| Err(borsa_core::BorsaError::unsupported("fundamentals/earnings")),
        )
    }
    /// Clone as `Arc<dyn YfOptions>`.
    fn clone_arc_options(&self) -> Arc<dyn YfOptions> {
        <dyn YfOptions>::from_fns(
            |_symbol| Err(BorsaError::unsupported("options/expirations")),
            |_symbol, _date| Err(BorsaError::unsupported("options/chain")),
        )
    }
    /// Clone as `Arc<dyn YfAnalysis>`.
    fn clone_arc_analysis(&self) -> Arc<dyn YfAnalysis> {
        <dyn YfAnalysis>::from_fns(
            |_s| Err(BorsaError::unsupported("analysis/recommendations")),
            |_s| Err(BorsaError::unsupported("analysis/recommendations_summary")),
            |_s| Err(BorsaError::unsupported("analysis/upgrades_downgrades")),
            |_s| Err(BorsaError::unsupported("analysis/price_target")),
        )
    }

    /// Clone as `Arc<dyn YfHolders>`.
    fn clone_arc_holders(&self) -> Arc<dyn YfHolders> {
        <dyn YfHolders>::from_fns(
            |_s| Err(BorsaError::unsupported("holders/major")),
            |_s| Err(BorsaError::unsupported("holders/institutional")),
            |_s| Err(BorsaError::unsupported("holders/mutual_fund")),
            |_s| Err(BorsaError::unsupported("holders/insider_transactions")),
            |_s| Err(BorsaError::unsupported("holders/insider_roster")),
            |_s| Err(BorsaError::unsupported("holders/net_share_purchase")),
        )
    }
    /// Clone as `Arc<dyn YfEsg>`.
    fn clone_arc_esg(&self) -> Arc<dyn YfEsg> {
        <dyn YfEsg>::from_fn(|_s| Err(BorsaError::unsupported("sustainability/esg")))
    }
    /// Clone as `Arc<dyn YfNews>`.
    fn clone_arc_news(&self) -> Arc<dyn YfNews> {
        <dyn YfNews>::from_fn(|_s, _r| Err(BorsaError::unsupported("news")))
    }
    /// Clone as `Arc<dyn YfStream>`.
    fn clone_arc_stream(&self) -> Arc<dyn YfStream> {
        struct Unsupported;
        #[async_trait]
        impl YfStream for Unsupported {
            /// Start is unsupported in the default stub.
            async fn start(
                &self,
                _symbols: &[String],
            ) -> Result<
                (
                    borsa_core::stream::StreamHandle,
                    tokio::sync::mpsc::Receiver<borsa_core::QuoteUpdate>,
                ),
                BorsaError,
            > {
                Err(BorsaError::unsupported("stream"))
            }
        }
        Arc::new(Unsupported)
    }
}

#[cfg(feature = "test-adapters")]
impl CloneArcAdapters for RealAdapter {
    fn clone_arc_history(&self) -> Arc<dyn YfHistory> {
        Arc::new(self.clone()) as Arc<dyn YfHistory>
    }
    fn clone_arc_quotes(&self) -> Arc<dyn YfQuotes> {
        Arc::new(self.clone()) as Arc<dyn YfQuotes>
    }
    fn clone_arc_search(&self) -> Arc<dyn YfSearch> {
        Arc::new(self.clone()) as Arc<dyn YfSearch>
    }
    fn clone_arc_profile(&self) -> Arc<dyn YfProfile> {
        Arc::new(self.clone()) as Arc<dyn YfProfile>
    }
    fn clone_arc_fundamentals(&self) -> Arc<dyn YfFundamentals> {
        Arc::new(self.clone()) as Arc<dyn YfFundamentals>
    }
    fn clone_arc_options(&self) -> Arc<dyn YfOptions> {
        Arc::new(self.clone()) as Arc<dyn YfOptions>
    }
    fn clone_arc_analysis(&self) -> Arc<dyn YfAnalysis> {
        Arc::new(self.clone()) as Arc<dyn YfAnalysis>
    }
    fn clone_arc_holders(&self) -> Arc<dyn YfHolders> {
        Arc::new(self.clone()) as Arc<dyn YfHolders>
    }
    fn clone_arc_esg(&self) -> Arc<dyn YfEsg> {
        Arc::new(self.clone()) as Arc<dyn YfEsg>
    }
    fn clone_arc_news(&self) -> Arc<dyn YfNews> {
        Arc::new(self.clone()) as Arc<dyn YfNews>
    }
    fn clone_arc_stream(&self) -> Arc<dyn YfStream> {
        Arc::new(self.clone()) as Arc<dyn YfStream>
    }
}
