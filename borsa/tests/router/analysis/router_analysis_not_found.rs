use crate::helpers::X;
use async_trait::async_trait;
use borsa::Borsa;
use borsa_core::connector::{BorsaConnector, RecommendationsSummaryProvider};
use borsa_core::{AssetKind, BorsaError, Instrument};

struct NF;
#[async_trait]
impl RecommendationsSummaryProvider for NF {
    async fn recommendations_summary(
        &self,
        _i: &Instrument,
    ) -> Result<borsa_core::RecommendationSummary, BorsaError> {
        Err(BorsaError::not_found("analysis for X"))
    }
}

#[async_trait]
impl BorsaConnector for NF {
    fn name(&self) -> &'static str {
        "nf"
    }

    fn supports_kind(&self, _kind: AssetKind) -> bool {
        true
    }

    fn as_recommendations_summary_provider(
        &self,
    ) -> Option<&dyn borsa_core::connector::RecommendationsSummaryProvider> {
        Some(self as &dyn RecommendationsSummaryProvider)
    }
}

#[tokio::test]
async fn all_not_found_returns_not_found() {
    let borsa = Borsa::builder()
        .with_connector(std::sync::Arc::new(NF))
        .build()
        .unwrap();
    let inst = crate::helpers::instrument(&X, AssetKind::Equity);
    let err = borsa.recommendations_summary(&inst).await.err().unwrap();
    assert!(matches!(err, BorsaError::NotFound { .. }));
}
