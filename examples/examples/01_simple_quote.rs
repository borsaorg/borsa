use borsa::Borsa;
use borsa_core::{AssetKind, Instrument};
use borsa_examples::common::get_connector;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create connector (mock in CI when BORSA_EXAMPLES_USE_MOCK is set).
    let connector = get_connector();

    // 2. Build the Borsa router and register the connector.
    let borsa = Borsa::builder().with_connector(connector).build()?;

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
