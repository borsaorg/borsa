use borsa::Borsa;
use borsa_core::{AssetKind, Instrument};
use borsa_yfinance::YfConnector;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create connectors. We'll use both Yahoo Finance and Alpha Vantage.
    let yf_connector = Arc::new(YfConnector::new_default());

    // 2. Build the Borsa router and register the connectors.
    let borsa = Borsa::builder().with_connector(yf_connector).build()?;

    // 3. Define the instrument we want to query.
    let instrument =
        Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid instrument symbol");

    // 4. Fetch the quote. Borsa handles the routing and fallback.
    println!("Fetching quote for {}...", instrument.symbol());
    let quote = borsa.quote(&instrument).await?;

    // 5. Print the result.
    println!("{quote:#?}");

    Ok(())
}
