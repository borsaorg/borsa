mod common;
#[cfg(feature = "dataframe")]
use borsa::Borsa;
#[cfg(feature = "dataframe")]
use borsa_core::{AssetKind, Instrument};

#[cfg(feature = "dataframe")]
use borsa_core::ToDataFrame;

#[cfg(feature = "dataframe")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let connector = common::get_connector();
    let borsa = Borsa::builder().with_connector(connector).build()?;

    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity)?;
    let quote = borsa.quote(&inst).await?;
    let df = quote.to_dataframe()?;
    println!(
        "DataFrame shape: {} rows x {} cols",
        df.height(),
        df.width()
    );
    Ok(())
}

#[cfg(not(feature = "dataframe"))]
fn main() {
    eprintln!("This example requires the 'dataframe' feature. Skipping.");
}
