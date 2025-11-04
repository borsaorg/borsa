use borsa::Borsa;
use borsa_core::{AssetKind, BorsaConnector, Instrument};
use borsa_middleware::QuotaAwareConnector;
use borsa_types::{QuotaConfig, QuotaConsumptionStrategy};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Wrap a real connector with a quota-aware middleware.
    let inner: Arc<dyn BorsaConnector> = Arc::new(borsa_yfinance::YfConnector::new_raw());
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
