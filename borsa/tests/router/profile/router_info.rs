use crate::helpers::{X, usd};
use borsa::Borsa;
use borsa_core::{AssetKind, BorsaError, Instrument, Isin, Profile, Symbol};
use borsa_core::{CompanyProfile, PriceTarget, Quote, RecommendationSummary};
use rust_decimal::Decimal;
use std::sync::Arc;

// A more comprehensive mock connector for this test
#[derive(Default)]
struct InfoConnector;

#[async_trait::async_trait]
impl borsa_core::connector::QuoteProvider for InfoConnector {
    async fn quote(&self, _i: &Instrument) -> Result<Quote, BorsaError> {
        Ok(Quote {
            symbol: borsa_core::Symbol::new("TEST").unwrap(),
            shortname: Some("Test Inc.".into()),
            price: Some(usd("150.0")),
            previous_close: None,
            exchange: None,
            market_state: None,
            day_volume: None,
        })
    }
}
#[async_trait::async_trait]
impl borsa_core::connector::ProfileProvider for InfoConnector {
    async fn profile(&self, _i: &Instrument) -> Result<Profile, BorsaError> {
        Ok(Profile::Company(CompanyProfile {
            name: "Test Incorporated".into(),
            summary: None,
            website: None,
            address: None,
            sector: Some("Tech".into()),
            industry: None,
            isin: Some(Isin::new("US0378331005").unwrap()),
        }))
    }
}
#[async_trait::async_trait]
impl borsa_core::connector::AnalystPriceTargetProvider for InfoConnector {
    async fn analyst_price_target(&self, _i: &Instrument) -> Result<PriceTarget, BorsaError> {
        Ok(PriceTarget {
            low: None,
            mean: Some(usd("180.0")),
            high: None,
            number_of_analysts: None,
        })
    }
}
#[async_trait::async_trait]
impl borsa_core::connector::RecommendationsSummaryProvider for InfoConnector {
    async fn recommendations_summary(
        &self,
        _i: &Instrument,
    ) -> Result<RecommendationSummary, BorsaError> {
        Ok(RecommendationSummary {
            latest_period: None,
            strong_buy: None,
            buy: None,
            hold: None,
            sell: None,
            strong_sell: None,
            mean: Some(1.8),
            mean_rating_text: None,
        })
    }
}
#[async_trait::async_trait]
impl borsa_core::connector::EsgProvider for InfoConnector {
    async fn sustainability(&self, _i: &Instrument) -> Result<borsa_core::EsgScores, BorsaError> {
        Err(BorsaError::unsupported("esg"))
    }
}

#[async_trait::async_trait]
impl borsa_core::BorsaConnector for InfoConnector {
    fn name(&self) -> &'static str {
        "info_conn"
    }

    fn supports_kind(&self, _kind: AssetKind) -> bool {
        true
    }

    fn as_quote_provider(&self) -> Option<&dyn borsa_core::connector::QuoteProvider> {
        Some(self)
    }
    fn as_profile_provider(&self) -> Option<&dyn borsa_core::connector::ProfileProvider> {
        Some(self)
    }
    fn as_analyst_price_target_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::AnalystPriceTargetProvider> {
        None
    }
    fn as_recommendations_summary_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::RecommendationsSummaryProvider> {
        None
    }
    fn as_esg_provider(&self) -> Option<&dyn borsa_core::connector::EsgProvider> {
        None
    }
}

#[tokio::test]
async fn router_info_aggregates_data() {
    let borsa = Borsa::builder()
        .with_connector(Arc::new(InfoConnector))
        .build()
        .unwrap();
    let test = Symbol::new("TEST").expect("valid symbol");
    let inst = crate::helpers::instrument(&test, AssetKind::Equity);

    let report = borsa.info(&inst).await.unwrap();
    let info = report.info.unwrap();

    // From quote
    assert_eq!(
        info.last.as_ref().map(borsa_core::Money::amount),
        Some(Decimal::from(150u8))
    );
    // From profile, falling back from quote's short_name
    assert_eq!(info.name.as_deref(), Some("Test Inc."));
    // From profile
    assert_eq!(info.isin, Some(Isin::new("US0378331005").unwrap()));
    // sector not part of aggregates Info; ensure identity fields present
    assert_eq!(info.symbol.as_str(), "TEST");
    // From analysis
    // analysis fields optional in Info; presence depends on provider coverage
}

#[tokio::test]
async fn router_fast_info_works() {
    let borsa = Borsa::builder()
        .with_connector(Arc::new(InfoConnector))
        .build()
        .unwrap();
    let test = Symbol::new("TEST").expect("valid symbol");
    let inst = crate::helpers::instrument(&test, AssetKind::Equity);

    let fast_info = borsa.fast_info(&inst).await.unwrap();
    assert_eq!(fast_info.last.unwrap().amount(), Decimal::from(150u8));
    assert_eq!(fast_info.symbol.as_str(), "TEST");
}

#[derive(Default)]
struct QuoteOnlyConnector;

#[async_trait::async_trait]
impl borsa_core::connector::QuoteProvider for QuoteOnlyConnector {
    async fn quote(&self, _i: &Instrument) -> Result<Quote, BorsaError> {
        Ok(Quote {
            symbol: borsa_core::Symbol::new("TEST").unwrap(),
            shortname: Some("Only Quote Inc.".into()),
            price: Some(usd("150.0")),
            previous_close: None,
            exchange: None,
            market_state: None,
            day_volume: None,
        })
    }
}

#[async_trait::async_trait]
impl borsa_core::BorsaConnector for QuoteOnlyConnector {
    fn name(&self) -> &'static str {
        "quote_only"
    }

    fn supports_kind(&self, _kind: AssetKind) -> bool {
        true
    }

    fn as_quote_provider(&self) -> Option<&dyn borsa_core::connector::QuoteProvider> {
        Some(self)
    }
}

#[derive(Default)]
struct FailingProfileAndPtConnector;

#[async_trait::async_trait]
impl borsa_core::connector::ProfileProvider for FailingProfileAndPtConnector {
    async fn profile(&self, _i: &Instrument) -> Result<Profile, BorsaError> {
        Err(BorsaError::Other("profile failed".into()))
    }
}

#[async_trait::async_trait]
impl borsa_core::connector::AnalystPriceTargetProvider for FailingProfileAndPtConnector {
    async fn analyst_price_target(&self, _i: &Instrument) -> Result<PriceTarget, BorsaError> {
        Err(BorsaError::Other("price target failed".into()))
    }
}

#[async_trait::async_trait]
impl borsa_core::BorsaConnector for FailingProfileAndPtConnector {
    fn name(&self) -> &'static str {
        "fail_profile_pt"
    }

    fn supports_kind(&self, _kind: AssetKind) -> bool {
        true
    }

    fn as_profile_provider(&self) -> Option<&dyn borsa_core::connector::ProfileProvider> {
        Some(self)
    }
    fn as_analyst_price_target_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::AnalystPriceTargetProvider> {
        None
    }
}

#[tokio::test]
async fn router_info_partial_failures_quote_ok_profile_and_target_fail() {
    let borsa = Borsa::builder()
        .with_connector(Arc::new(QuoteOnlyConnector))
        .with_connector(Arc::new(FailingProfileAndPtConnector))
        .build()
        .unwrap();
    let test = Symbol::new("TEST").expect("valid symbol");
    let inst = crate::helpers::instrument(&test, AssetKind::Equity);

    let report = borsa.info(&inst).await.unwrap();
    let info = report.info.unwrap();

    // Quote fields still present
    assert_eq!(
        info.last.as_ref().map(borsa_core::Money::amount),
        Some(Decimal::from(150u8))
    );
    assert_eq!(info.name.as_deref(), Some("Only Quote Inc."));

    // Profile- and analysis-derived fields absent due to failures
    // aggregates Info omits these fields by design; presence is not required
}

#[derive(Default)]
struct MinimalInfoConnector;

#[async_trait::async_trait]
impl borsa_core::connector::QuoteProvider for MinimalInfoConnector {
    async fn quote(&self, _i: &Instrument) -> Result<Quote, BorsaError> {
        let _inst = crate::helpers::instrument(&X, AssetKind::Equity);
        Ok(Quote {
            symbol: X.clone(),
            shortname: Some("Minimal Inc.".into()),
            price: Some(usd("42.0")),
            previous_close: None,
            exchange: None,
            market_state: None,
            day_volume: None,
        })
    }
}

#[async_trait::async_trait]
impl borsa_core::connector::ProfileProvider for MinimalInfoConnector {
    async fn profile(&self, _i: &Instrument) -> Result<Profile, BorsaError> {
        Ok(Profile::Company(CompanyProfile {
            name: "Minimal Incorporated".into(),
            summary: None,
            website: None,
            address: None,
            sector: None,
            industry: None,
            isin: None,
        }))
    }
}

#[async_trait::async_trait]
impl borsa_core::connector::AnalystPriceTargetProvider for MinimalInfoConnector {
    async fn analyst_price_target(&self, _i: &Instrument) -> Result<PriceTarget, BorsaError> {
        panic!("info() should not call analyst_price_target");
    }
}

#[async_trait::async_trait]
impl borsa_core::connector::RecommendationsSummaryProvider for MinimalInfoConnector {
    async fn recommendations_summary(
        &self,
        _i: &Instrument,
    ) -> Result<RecommendationSummary, BorsaError> {
        panic!("info() should not call recommendations_summary");
    }
}

#[async_trait::async_trait]
impl borsa_core::connector::EsgProvider for MinimalInfoConnector {
    async fn sustainability(&self, _i: &Instrument) -> Result<borsa_core::EsgScores, BorsaError> {
        panic!("info() should not call sustainability");
    }
}

#[async_trait::async_trait]
impl borsa_core::connector::IsinProvider for MinimalInfoConnector {
    async fn isin(&self, _i: &Instrument) -> Result<Option<Isin>, BorsaError> {
        Ok(None)
    }
}

#[async_trait::async_trait]
impl borsa_core::BorsaConnector for MinimalInfoConnector {
    fn name(&self) -> &'static str {
        "minimal_info"
    }

    fn supports_kind(&self, _kind: AssetKind) -> bool {
        true
    }

    fn as_quote_provider(&self) -> Option<&dyn borsa_core::connector::QuoteProvider> {
        Some(self)
    }
    fn as_profile_provider(&self) -> Option<&dyn borsa_core::connector::ProfileProvider> {
        Some(self)
    }
    fn as_analyst_price_target_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::AnalystPriceTargetProvider> {
        None
    }
    fn as_recommendations_summary_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::RecommendationsSummaryProvider> {
        None
    }
    fn as_esg_provider(&self) -> Option<&dyn borsa_core::connector::EsgProvider> {
        None
    }
    fn as_isin_provider(&self) -> Option<&dyn borsa_core::connector::IsinProvider> {
        Some(self)
    }
}

#[tokio::test]
async fn router_info_ignores_unused_capabilities() {
    let borsa = Borsa::builder()
        .with_connector(Arc::new(MinimalInfoConnector))
        .build()
        .unwrap();
    let inst = crate::helpers::instrument(&X, AssetKind::Equity);

    let report = borsa.info(&inst).await.unwrap();
    assert!(
        report.warnings.is_empty(),
        "unexpected warnings: {:?}",
        report.warnings
    );
    let info = report.info.unwrap();
    assert_eq!(info.symbol.as_str(), "X");
    assert_eq!(info.name.as_deref(), Some("Minimal Inc."));
    assert_eq!(
        info.last.as_ref().map(borsa_core::Money::amount),
        Some(Decimal::from(42u8))
    );
}

#[tokio::test]
async fn router_info_suppresses_optional_warnings() {
    let borsa = Borsa::builder()
        .with_connector(Arc::new(InfoConnector))
        .build()
        .unwrap();
    let test = Symbol::new("TEST").expect("valid symbol");
    let inst = crate::helpers::instrument(&test, AssetKind::Equity);

    let report = borsa.info(&inst).await.unwrap();
    assert!(
        report.warnings.is_empty(),
        "expected no warnings, got {:?}",
        report.warnings
    );
    let info = report.info.unwrap();
    assert_eq!(info.isin, Some(Isin::new("US0378331005").unwrap()));
}
