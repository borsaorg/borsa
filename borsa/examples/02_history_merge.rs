mod common;
use borsa::Borsa;
use borsa_core::{AssetKind, HistoryRequest, Instrument, connector::BorsaConnector};
use borsa_mock::MockConnector;
use common::get_connector;

use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create instances of our connectors.
    let yf_connector = get_connector();
    let mock_connector = Arc::new(MockConnector::new());

    // 2. Build Borsa and set a priority order for history.
    // We'll tell Borsa to prefer our mock connector for history data, then yfinance.
    let borsa = Borsa::builder()
        .with_connector(yf_connector.clone())
        .with_connector(mock_connector.clone())
        .routing_policy(
            borsa_core::RoutingPolicyBuilder::new()
                .providers_for_kind(
                    AssetKind::Equity,
                    &[mock_connector.key(), yf_connector.key()],
                )
                .build(),
        )
        .build()?;

    // 3. Define the instrument and a request for recent history.
    let instrument =
        Instrument::from_symbol("GOOG", AssetKind::Equity).expect("valid instrument symbol");
    let req =
        HistoryRequest::try_from_range(borsa_core::Range::D5, borsa_core::Interval::D1).unwrap();

    println!("Fetching 5-day history for {}...", instrument.symbol());
    println!(
        "Priority: [{}, {}]",
        mock_connector.name(),
        yf_connector.name()
    );

    // 4. Fetch history *with attribution* to see how Borsa merged the data.
    let (history, attribution) = borsa.history_with_attribution(&instrument, req).await?;

    // 5. Print the results.
    println!("\n## Merged History ({} candles):", history.candles.len());
    for candle in history.candles.iter().take(10) {
        // Print first 10
        println!(
            " - TS: {}, Close: ${:.2}",
            candle.ts.timestamp(),
            candle.close.amount()
        );
    }
    if history.candles.len() > 10 {
        println!("... and more");
    }

    println!("\n## Data Attribution:");
    for (name, span) in &attribution.spans {
        println!(
            " - Connector '{}' provided data from TS {} to {}.",
            name, span.start, span.end
        );
    }

    Ok(())
}
