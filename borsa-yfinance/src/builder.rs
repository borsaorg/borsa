use std::sync::Arc;
use std::time::Duration;

use borsa_core::connector::BorsaConnector;
use borsa_middleware::ConnectorBuilder as GenericConnectorBuilder;
use borsa_types::{QuotaConfig, QuotaConsumptionStrategy};

use crate::YfConnector;

/// Builder type alias specialized for yfinance connectors.
pub type YfConnectorBuilder = GenericConnectorBuilder;

impl YfConnector {
    
    /// Returns an unconfigured builder with the default connector.
    /// 
    /// Customize with the builder methods before calling `.build()`. 
    #[must_use]
    pub fn new() -> YfConnectorBuilder {
        let raw: Arc<dyn BorsaConnector> = Arc::new(Self::new_default());
        GenericConnectorBuilder::new(raw)
    }
    
    /// Returns a builder with a conservative rate limit (1 request every ~4 seconds).
    ///
    /// Users can further customize before calling `.build()`.
    #[must_use]
    pub fn rate_limited() -> YfConnectorBuilder {
        let raw: Arc<dyn BorsaConnector> = Arc::new(Self::new_default());
        let cfg = QuotaConfig {
            // 15 per minute -> ~1 per 4 seconds when evenly spread
            limit: 15,
            window: Duration::from_secs(60),
            strategy: QuotaConsumptionStrategy::EvenSpreadHourly,
        };
        GenericConnectorBuilder::new(raw)
            .with_quota(&cfg)
            .with_blacklist(Duration::from_secs(5 * 60))
    }

    /// Expert-only: construct an unwrapped connector for manual composition.
    #[must_use]
    pub fn new_raw() -> Self {
        Self::new_default()
    }
}
