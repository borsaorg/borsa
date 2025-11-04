#![cfg(feature = "dataframe")]

mod common;
use borsa::Borsa;
use borsa_core::{AssetKind, Instrument, ToDataFrame};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Run with: cargo run -p borsa --features dataframe --example 23_dataframe
    let connector = common::get_connector();
    let borsa = Borsa::builder().with_connector(connector).build()?;

    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity)?;
    let quote = borsa.quote(&inst).await?;
    let df = quote.to_dataframe()?; // -> polars::DataFrame
    println!(
        "DataFrame shape: {} rows x {} cols",
        df.height(),
        df.width()
    );
    Ok(())
}
