use async_trait::async_trait;
use borsa::Borsa;
use borsa_core::{
    AssetKind, BorsaError, Candle, Currency, HistoryRequest, HistoryResponse, Instrument, Money,
    connector::{BorsaConnector, HistoryProvider},
};
use borsa_yfinance::YfConnector;
use chrono::TimeZone;
use std::sync::Arc;

/// A simple mock connector to demonstrate merging.
/// It provides a short, predefined history for a specific symbol.
struct MockConnector;

#[async_trait]
impl BorsaConnector for MockConnector {
    fn name(&self) -> &'static str {
        "mock-connector"
    }

    fn as_history_provider(&self) -> Option<&dyn HistoryProvider> {
        Some(self as &dyn HistoryProvider)
    }
}

#[async_trait]
impl HistoryProvider for MockConnector {
    async fn history(
        &self,
        _i: &Instrument,
        _r: HistoryRequest,
    ) -> Result<HistoryResponse, BorsaError> {
        println!("-> MockConnector providing its historical data...");
        Ok(HistoryResponse {
            candles: vec![
                Candle {
                    ts: chrono::Utc.timestamp_opt(1_723_507_200, 0).unwrap(),
                    open: Money::from_canonical_str(
                        "10.0",
                        Currency::Iso(borsa_core::IsoCurrency::USD),
                    )
                    .unwrap(),
                    high: Money::from_canonical_str(
                        "12.0",
                        Currency::Iso(borsa_core::IsoCurrency::USD),
                    )
                    .unwrap(),
                    low: Money::from_canonical_str(
                        "9.0",
                        Currency::Iso(borsa_core::IsoCurrency::USD),
                    )
                    .unwrap(),
                    close: Money::from_canonical_str(
                        "11.5",
                        Currency::Iso(borsa_core::IsoCurrency::USD),
                    )
                    .unwrap(),
                    close_unadj: None,
                    volume: Some(1000),
                }, // 2024-08-13
                Candle {
                    ts: chrono::Utc.timestamp_opt(1_723_593_600, 0).unwrap(),
                    open: Money::from_canonical_str(
                        "11.5",
                        Currency::Iso(borsa_core::IsoCurrency::USD),
                    )
                    .unwrap(),
                    high: Money::from_canonical_str(
                        "14.0",
                        Currency::Iso(borsa_core::IsoCurrency::USD),
                    )
                    .unwrap(),
                    low: Money::from_canonical_str(
                        "11.0",
                        Currency::Iso(borsa_core::IsoCurrency::USD),
                    )
                    .unwrap(),
                    close: Money::from_canonical_str(
                        "13.5",
                        Currency::Iso(borsa_core::IsoCurrency::USD),
                    )
                    .unwrap(),
                    close_unadj: None,
                    volume: Some(1200),
                }, // 2024-08-14
            ],
            actions: vec![],
            adjusted: false,
            meta: None,
        })
    }

    fn supported_history_intervals(&self, _i: AssetKind) -> &'static [borsa_core::Interval] {
        &[borsa_core::Interval::D1]
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create instances of our connectors.
    let yf_connector = Arc::new(YfConnector::new_default());
    let mock_connector = Arc::new(MockConnector);


    // 2. Build Borsa and set a priority order for history.
    // We'll tell Borsa to prefer our mock connector for history data, then yfinance, then Alpha Vantage.
    let borsa = Borsa::builder()
        .with_connector(yf_connector.clone())
        .with_connector(mock_connector.clone())
        .prefer_for_kind(AssetKind::Equity, &[mock_connector, yf_connector])
        .build();

    // 3. Define the instrument and a request for recent history.
    let instrument =
        Instrument::from_symbol("GOOG", AssetKind::Equity).expect("valid instrument symbol");
    let req =
        HistoryRequest::try_from_range(borsa_core::Range::D5, borsa_core::Interval::D1).unwrap();

    println!("Fetching 5-day history for {}...", instrument.symbol());
    println!("Priority: [mock-connector, borsa-yfinance]");

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
