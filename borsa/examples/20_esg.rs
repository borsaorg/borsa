mod common;
use borsa::Borsa;
use borsa_core::{AssetKind, Instrument};

/// Unfortunately, the yfinance connector does not support ESG scores anymore
/// so this example will not work with the bundled yfinance connector.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let connector = common::get_connector();
    let borsa = Borsa::builder().with_connector(connector).build()?;

    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity)?;
    println!("Fetching ESG scores for {}...", inst.symbol());

    match borsa.sustainability(&inst).await {
        Ok(scores) => {
            println!(
                "E: {:?}, S: {:?}, G: {:?}",
                scores.environmental, scores.social, scores.governance
            );
        }
        Err(e) => {
            eprintln!("ESG not available: {e}");
        }
    }

    Ok(())
}
