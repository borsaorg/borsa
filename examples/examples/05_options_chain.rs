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
        Instrument::from_symbol("AMD", AssetKind::Equity).expect("valid instrument symbol");
    println!("Fetching option expirations for {}...", instrument.symbol());

    // 3. Get the list of available expiration dates (as Unix timestamps).
    let expirations = borsa.options_expirations(&instrument).await?;

    if let Some(&next_expiry) = expirations.first() {
        println!(
            "Found {} expiration dates. Fetching chain for nearest date: {}...",
            expirations.len(),
            next_expiry
        );

        // 4. Fetch the full option chain for the nearest expiration date.
        let chain = borsa.option_chain(&instrument, Some(next_expiry)).await?;

        println!(
            "\n## Option Chain for {} (Expires {})",
            instrument.symbol(),
            next_expiry
        );
        println!("- Found {} call options.", chain.calls.len());
        println!("- Found {} put options.", chain.puts.len());

        // 5. Display a few call options from the chain.
        println!("\n--- Sample Call Options ---");
        println!(
            "{:<22} | {:<8} | {:<8} | {:<8} | Ask",
            "Contract Symbol", "Strike", "Last", "Bid"
        );
        println!(
            "{:-<23}|{:-<10}|{:-<10}|{:-<10}|{:-<10}",
            "", "", "", "", ""
        );

        for call in chain.calls.iter().take(5) {
            println!(
                "{:<22} | ${:<7.2} | ${:<7.2} | ${:<7.2} | ${:<7.2}",
                call.contract_symbol,
                call.strike.amount(),
                call.price
                    .as_ref()
                    .map(borsa_core::Money::amount)
                    .unwrap_or_default(),
                call.bid
                    .as_ref()
                    .map(borsa_core::Money::amount)
                    .unwrap_or_default(),
                call.ask
                    .as_ref()
                    .map(borsa_core::Money::amount)
                    .unwrap_or_default()
            );
        }
    } else {
        println!(
            "No option expiration dates found for {}.",
            instrument.symbol()
        );
    }

    Ok(())
}
