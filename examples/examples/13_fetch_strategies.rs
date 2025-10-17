use std::sync::Arc;

use borsa::Borsa;
use borsa::FetchStrategy;
use borsa_core::{AssetKind, Instrument};
use borsa_yfinance::YfConnector;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let yf = Arc::new(YfConnector::new_default());

    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid instrument symbol");

    // Default: PriorityWithFallback with 5s timeout
    let borsa_default = Borsa::builder().with_connector(yf.clone()).build()?;
    let _ = borsa_default.quote(&inst).await?;

    // Explicitly set sequential fallback with a tighter timeout
    let borsa_seq = Borsa::builder()
        .with_connector(yf.clone())
        .fetch_strategy(FetchStrategy::PriorityWithFallback)
        .provider_timeout(std::time::Duration::from_millis(800))
        .build()?;
    let _ = borsa_seq.quote(&inst).await?;

    // Latency-first: fire all providers concurrently and take first success
    let borsa_latency = Borsa::builder()
        .with_connector(yf.clone())
        .fetch_strategy(FetchStrategy::Latency)
        .build()?;
    let _ = borsa_latency.quote(&inst).await?;

    println!("Examples ran successfully.");
    Ok(())
}
