use borsa_core::{BorsaConnector, QuoteUpdate, Symbol};
use borsa_mock::{DynamicMockConnector, StreamBehavior};
use std::sync::Arc;

#[must_use]
pub fn get_connector() -> Arc<dyn BorsaConnector> {
    if std::env::var("BORSA_EXAMPLES_USE_MOCK").is_ok() {
        // Use dynamic mock only for the streaming example; else use the static fixtures mock.
        let is_streaming_example = std::env::args()
            .next()
            .is_some_and(|p| p.contains("17_streaming"));
        if is_streaming_example {
            println!("--- (Using Dynamic Mock Connector for streaming example) ---");
            // Prepare 20 scripted updates with increasing timestamps.
            let sym = Symbol::new("AAPL").unwrap();
            let start = chrono::Utc::now();
            let updates: Vec<QuoteUpdate> = (0..20)
                .map(|i| QuoteUpdate {
                    symbol: sym.clone(),
                    price: None,
                    previous_close: None,
                    ts: start + chrono::TimeDelta::seconds(i),
                    volume: None,
                })
                .collect();
            let (connector, _ctrl) = DynamicMockConnector::new_with_controller_and_behavior(
                "examples-mock",
                StreamBehavior::Success(updates),
            );
            connector
        } else {
            println!("--- (Using Static Mock Connector for CI) ---");
            Arc::new(borsa_mock::MockConnector::new())
        }
    } else {
        // Use the raw connector to disable the rate limiting middleware
        Arc::new(borsa_yfinance::YfConnector::new_raw())
    }
}
