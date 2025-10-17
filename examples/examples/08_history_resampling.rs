use borsa::Borsa;
use borsa_core::{AssetKind, HistoryRequest, Instrument, Interval, Range};
use borsa_yfinance::YfConnector;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Setup Borsa with a special configuration to resample history to weekly.
    let yf_connector = Arc::new(YfConnector::new_default());
    let borsa = Borsa::builder()
        .with_connector(yf_connector)
        .resampling(borsa::Resampling::Weekly) // Enable weekly resampling!
        .build()?;

    // 2. Define instrument and request daily data for the past year.
    let instrument =
        Instrument::from_symbol("TSLA", AssetKind::Equity).expect("valid instrument symbol");
    let req = HistoryRequest::try_from_range(Range::Y1, Interval::D1).unwrap();

    println!(
        "Fetching daily history for {} and resampling to weekly...",
        instrument.symbol()
    );

    // 3. Fetch the history. Borsa will automatically process it.
    let history = borsa.history(&instrument, req).await?;

    // 4. Print the first few weekly candles.
    println!("\n## Resampled Weekly Candles:");
    println!(
        "{:<12} | {:<10} | {:<10} | {:<10} | {:<10}",
        "Week Start", "Open", "High", "Low", "Close"
    );
    println!(
        "{:-<13}|{:-<12}|{:-<12}|{:-<12}|{:-<12}",
        "", "", "", "", ""
    );

    for candle in history.candles.iter().take(10) {
        // The timestamp (ts) will be the Monday of that week.
        let date = candle.ts.format("%Y-%m-%d");
        println!(
            "{:<12} | ${:<9.2} | ${:<9.2} | ${:<9.2} | ${:<9.2}",
            date,
            candle.open.amount(),
            candle.high.amount(),
            candle.low.amount(),
            candle.close.amount()
        );
    }

    Ok(())
}
