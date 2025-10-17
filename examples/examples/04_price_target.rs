use borsa::Borsa;
use borsa_core::{AssetKind, Instrument};
use borsa_yfinance::YfConnector;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Setup Borsa with the yfinance connector.
    let yf_connector = Arc::new(YfConnector::new_default());
    let borsa = Borsa::builder().with_connector(yf_connector).build();

    // 2. Define the instrument.
    let instrument =
        Instrument::from_symbol("NVDA", AssetKind::Equity).expect("valid instrument symbol");

    println!(
        "Fetching analyst price target for {}...",
        instrument.symbol()
    );

    // 3. Fetch the price target data.
    let target = borsa.analyst_price_target(&instrument).await?;

    // 4. Print a formatted summary.
    println!("\n## Analyst Price Target for {}", instrument.symbol());
    if let (Some(low), Some(mean), Some(high), Some(count)) = (
        target.low,
        target.mean,
        target.high,
        target.number_of_analysts,
    ) {
        println!("- Based on {count} analysts:");
        println!("  - High:   ${:.2}", high.amount());
        println!("  - Mean:   ${:.2}", mean.amount());
        println!("  - Low:    ${:.2}", low.amount());
    } else {
        println!("- No complete analyst price target data available.");
    }

    Ok(())
}
