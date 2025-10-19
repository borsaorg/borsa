use borsa_core::BorsaConnector;
use borsa_yfinance::YfConnector;

#[test]
fn yf_connector_advertises_all_capabilities() {
    let yf = YfConnector::new_default();
    assert!(yf.as_quote_provider().is_some());
    assert!(yf.as_history_provider().is_some());
    assert!(yf.as_search_provider().is_some());
    assert!(yf.as_profile_provider().is_some());
    assert!(yf.as_earnings_provider().is_some());
    assert!(yf.as_income_statement_provider().is_some());
    assert!(yf.as_balance_sheet_provider().is_some());
    assert!(yf.as_cashflow_provider().is_some());
    assert!(yf.as_calendar_provider().is_some());
    assert!(yf.as_options_expirations_provider().is_some());
    assert!(yf.as_option_chain_provider().is_some());
    assert!(yf.as_recommendations_provider().is_some());
    assert!(yf.as_recommendations_summary_provider().is_some());
    assert!(yf.as_upgrades_downgrades_provider().is_some());
    assert!(yf.as_analyst_price_target_provider().is_some());
    assert!(yf.as_major_holders_provider().is_some());
    assert!(yf.as_institutional_holders_provider().is_some());
    assert!(yf.as_mutual_fund_holders_provider().is_some());
    assert!(yf.as_insider_transactions_provider().is_some());
    assert!(yf.as_insider_roster_holders_provider().is_some());
    assert!(yf.as_net_share_purchase_activity_provider().is_some());
    assert!(yf.as_esg_provider().is_some());
    assert!(yf.as_news_provider().is_some());
    assert!(yf.as_isin_provider().is_some());
}
