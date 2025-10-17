use borsa::Borsa;
use borsa_core::{AssetKind, Instrument};
use borsa_yfinance::YfConnector;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Setup Borsa.
    let yf_connector = Arc::new(YfConnector::new_default());
    let borsa = Borsa::builder().with_connector(yf_connector).build()?;

    // 2. Define the two instruments we want to compare.
    let coke = Instrument::from_symbol("KO", AssetKind::Equity).expect("valid instrument symbol");
    let pepsi = Instrument::from_symbol("PEP", AssetKind::Equity).expect("valid instrument symbol");
    println!(
        "Fetching quotes for {} vs {}...",
        coke.symbol(),
        pepsi.symbol()
    );

    // 3. Fetch both quotes concurrently.
    let (coke_res, pepsi_res) = tokio::join!(borsa.quote(&coke), borsa.quote(&pepsi));

    // 4. Print the results in a comparison table.
    println!("\n## Market Comparison");
    println!(
        "{:<15} | {:<15} | {:<15}",
        "Metric",
        coke.symbol(),
        pepsi.symbol()
    );
    println!("{:-<16}|{:-<17}|{:-<17}", "", "", "");

    let coke_price = coke_res
        .as_ref()
        .ok()
        .and_then(|q| q.price.as_ref().map(borsa_core::Money::amount))
        .unwrap_or_default();
    let pepsi_price = pepsi_res
        .as_ref()
        .ok()
        .and_then(|q| q.price.as_ref().map(borsa_core::Money::amount))
        .unwrap_or_default();
    println!(
        "{:<15} | ${:<14.2} | ${:<14.2}",
        "Last Price", coke_price, pepsi_price
    );

    let coke_prev_close = coke_res
        .as_ref()
        .ok()
        .and_then(|q| q.previous_close.as_ref().map(borsa_core::Money::amount))
        .unwrap_or_default();
    let pepsi_prev_close = pepsi_res
        .as_ref()
        .ok()
        .and_then(|q| q.previous_close.as_ref().map(borsa_core::Money::amount))
        .unwrap_or_default();
    println!(
        "{:<15} | ${:<14.2} | ${:<14.2}",
        "Previous Close", coke_prev_close, pepsi_prev_close
    );

    let coke_change = coke_price - coke_prev_close;
    let pepsi_change = pepsi_price - pepsi_prev_close;
    println!(
        "{:<15} | {:<15.2} | {:<15.2}",
        "Day's Change", coke_change, pepsi_change
    );

    Ok(())
}
