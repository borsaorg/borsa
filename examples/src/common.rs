use borsa_core::BorsaConnector;
use std::sync::Arc;

/// Return a connector for examples.
///
/// # Panics
/// Panics if the `YFinance` rate-limited builder fails middleware validation.
#[must_use]
pub fn get_connector() -> Arc<dyn BorsaConnector> {
    if std::env::var("BORSA_EXAMPLES_USE_MOCK").is_ok() {
        println!("--- (Using Mock Connector for CI) ---");
        Arc::new(borsa_mock::MockConnector::new())
    } else {
        borsa_yfinance::YfConnector::rate_limited()
            .build()
            .expect("middleware stack validation failed")
    }
}
