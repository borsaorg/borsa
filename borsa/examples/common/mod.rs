use borsa_core::BorsaConnector;
use std::sync::Arc;

#[must_use]
pub fn get_connector() -> Arc<dyn BorsaConnector> {
    if std::env::var("BORSA_EXAMPLES_USE_MOCK").is_ok() {
        println!("--- (Using Mock Connector for CI) ---");
        Arc::new(borsa_mock::MockConnector::new())
    } else {
        // Use the raw connector to disable the rate limiting middleware
        Arc::new(borsa_yfinance::YfConnector::new_raw())
    }
}
