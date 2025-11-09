mod common;
use borsa::Borsa;
use borsa_core::{AssetKind, Instrument};
use common::get_connector;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Setup Borsa with selected connector (mock in CI when BORSA_EXAMPLES_USE_MOCK is set).
    let connector = get_connector();
    let borsa = Borsa::builder().with_connector(connector).build()?;

    // 2. Define the instrument.
    let instrument =
        Instrument::from_symbol("AMD", AssetKind::Equity).expect("valid instrument symbol");
    let sym_str = match instrument.id() {
        borsa_core::IdentifierScheme::Security(sec) => sec.symbol.as_str(),
        borsa_core::IdentifierScheme::Prediction(_) => "<non-security>",
    };
    println!("Fetching option expirations for {sym_str}...");

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

        println!("\n## Option Chain for {sym_str} (Expires {next_expiry})");
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
        println!("No option expiration dates found for {sym_str}.");
    }

    Ok(())
}
