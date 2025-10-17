use borsa_core::BorsaConnector;
use borsa_yfinance::YfConnector;

#[test]
fn yf_connector_advertises_fundamentals_capability() {
    let yf = YfConnector::new_default();
    assert!(yf.as_earnings_provider().is_some());
    assert!(yf.as_income_statement_provider().is_some());
    assert!(yf.as_balance_sheet_provider().is_some());
    assert!(yf.as_cashflow_provider().is_some());
    assert!(yf.as_calendar_provider().is_some());
}
