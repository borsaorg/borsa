mod common;
use borsa::Borsa;
use borsa_core::{AssetKind, SearchRequest};
use common::get_connector;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Setup Borsa with selected connector (mock in CI when BORSA_EXAMPLES_USE_MOCK is set).
    let connector = get_connector();
    let borsa = Borsa::builder().with_connector(connector).build()?;

    // 2. Create a search request. We're looking for up to 5 equity results for "tesla".
    let request = SearchRequest::builder("tesla")
        .kind(AssetKind::Equity)
        .limit(5)
        .build()
        .unwrap();

    println!("Searching for 'tesla'...");

    // 3. Perform the search.
    let report = borsa.search(request).await?;

    // 4. Print the results in a formatted way.
    println!("\n## Search Results:");
    println!("{:<10} | {:<40} | Exchange", "Symbol", "Name");
    println!("{:-<11}|{:-<42}|{:-<15}", "", "", "");
    if let Some(resp) = report.response {
        for result in resp.results {
            let sym = match result.instrument.id() {
                borsa_core::IdentifierScheme::Security(sec) => sec.symbol.as_str().to_string(),
                borsa_core::IdentifierScheme::Prediction(_) => "<non-security>".to_string(),
            };
            println!(
                "{:<10} | {:<40} | {}",
                sym,
                result.name.unwrap_or_default(),
                result.exchange.map(|e| e.to_string()).unwrap_or_default()
            );
        }
    }

    Ok(())
}
