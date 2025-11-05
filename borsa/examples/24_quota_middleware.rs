mod common;
use borsa::Borsa;
use borsa_core::{AssetKind, BorsaConnector, Instrument};
use borsa_middleware::QuotaAwareConnector;
use borsa_types::{QuotaConfig, QuotaConsumptionStrategy};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Wrap the selected connector with a quota-aware middleware.
    // In CI, BORSA_EXAMPLES_USE_MOCK=1 will provide the mock connector via common::get_connector.
    let inner: Arc<dyn BorsaConnector> = common::get_connector();
    let cfg = QuotaConfig {
        limit: 1000,
        window: Duration::from_secs(24 * 60 * 60),
        strategy: QuotaConsumptionStrategy::Unit,
    };
    let wrapped = Arc::new(QuotaAwareConnector::new(inner, cfg));

    let borsa = Borsa::builder().with_connector(wrapped).build()?;

    let aapl = Instrument::from_symbol("AAPL", AssetKind::Equity)?;
    let q = borsa.quote(&aapl).await?;
    println!("fetched: {:?}", q.symbol);
    Ok(())
}
