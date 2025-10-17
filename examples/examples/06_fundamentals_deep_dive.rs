use borsa::Borsa;
use borsa_core::{AssetKind, Instrument};
use borsa_yfinance::YfConnector;
use rust_decimal::Decimal;
use std::sync::Arc;

// A helper to format large financial numbers from Money amounts
fn format_num(n: Option<borsa_core::Money>) -> String {
    n.map_or_else(
        || "N/A".to_string(),
        |m| {
            let val = m.amount();
            if val.abs() > Decimal::from(1_000_000_000_u64) {
                format!("{:.2}B", (val / Decimal::from(1_000_000_000_u64)))
            } else if val.abs() > Decimal::from(1_000_000_u64) {
                format!("{:.2}M", (val / Decimal::from(1_000_000_u64)))
            } else {
                format!("{:.2}K", (val / Decimal::from(1_000_u64)))
            }
        },
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Setup Borsa with the yfinance connector.
    let yf_connector = Arc::new(YfConnector::new_default());
    let borsa = Borsa::builder().with_connector(yf_connector).build()?;

    // 2. Define the instrument.
    let instrument =
        Instrument::from_symbol("MSFT", AssetKind::Equity).expect("valid instrument symbol");
    println!(
        "Fetching latest quarterly financials for {}...",
        instrument.symbol()
    );

    // 3. Fetch the latest quarterly Income Statement, Balance Sheet, and Cash Flow.
    let income_stmt = borsa.income_statement(&instrument, true).await?;
    let balance_sheet = borsa.balance_sheet(&instrument, true).await?;
    let cash_flow = borsa.cashflow(&instrument, true).await?;

    // 4. Print a summary of the latest reported quarter.
    println!(
        "\n## Latest Quarterly Financials for {}",
        instrument.symbol()
    );

    if let Some(latest_income) = income_stmt.first() {
        println!("\n--- Income Statement ---");
        println!(
            "  - Total Revenue:    {}",
            format_num(latest_income.total_revenue.clone())
        );
        println!(
            "  - Gross Profit:     {}",
            format_num(latest_income.gross_profit.clone())
        );
        println!(
            "  - Net Income:       {}",
            format_num(latest_income.net_income.clone())
        );
    }

    if let Some(latest_bs) = balance_sheet.first() {
        println!("\n--- Balance Sheet ---");
        println!(
            "  - Total Assets:     {}",
            format_num(latest_bs.total_assets.clone())
        );
        println!(
            "  - Total Liabilities:{}",
            format_num(latest_bs.total_liabilities.clone())
        );
        println!(
            "  - Cash:             {}",
            format_num(latest_bs.cash.clone())
        );
    }

    if let Some(latest_cf) = cash_flow.first() {
        println!("\n--- Cash Flow ---");
        println!(
            "  - Operating Cashflow: {}",
            format_num(latest_cf.operating_cashflow.clone())
        );
        println!(
            "  - Free Cash Flow:     {}",
            format_num(latest_cf.free_cash_flow.clone())
        );
    }

    Ok(())
}
