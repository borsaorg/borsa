mod common;
use borsa::Borsa;
use borsa_core::{AssetKind, Instrument, Money};
use common::get_connector;

fn fmt_money(v: Option<&Money>) -> String {
    v.as_ref()
        .map_or_else(|| "<none>".to_string(), |m| m.format())
}

fn fmt_date(ts: Option<chrono::DateTime<chrono::Utc>>) -> String {
    ts.map_or_else(
        || "<none>".to_string(),
        |d| d.format("%Y-%m-%d").to_string(),
    )
}

#[allow(clippy::too_many_lines)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1) Build Borsa with the examples connector (rate-limited YF by default; Mock in CI)
    let connector = get_connector();
    let borsa = Borsa::builder().with_connector(connector).build()?;

    // 2) Choose an instrument
    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity)?;
    println!("Fetching fundamentals for {}...", inst.symbol());

    // 3) Fetch all fundamentals concurrently
    let (
        earnings_res,
        calendar_res,
        inc_ann_res,
        inc_q_res,
        bs_ann_res,
        bs_q_res,
        cf_ann_res,
        cf_q_res,
    ) = tokio::join!(
        borsa.earnings(&inst),
        borsa.calendar(&inst),
        borsa.income_statement(&inst, false),
        borsa.income_statement(&inst, true),
        borsa.balance_sheet(&inst, false),
        borsa.balance_sheet(&inst, true),
        borsa.cashflow(&inst, false),
        borsa.cashflow(&inst, true),
    );

    println!("\n========================================");
    println!("Fundamentals Deep Dive for {}", inst.symbol());
    println!("========================================\n");

    // Earnings
    println!("## Earnings");
    match earnings_res {
        Ok(e) => {
            if let Some(latest) = e.yearly.last() {
                println!(
                    "Latest Annual ({}): revenue={}, earnings={}",
                    latest.year,
                    fmt_money(latest.revenue.as_ref()),
                    fmt_money(latest.earnings.as_ref())
                );
            }
            if !e.quarterly_eps.is_empty() {
                println!("Recent Quarterly EPS (actual vs estimate):");
                for row in e.quarterly_eps.iter().rev().take(4) {
                    let act = row
                        .actual
                        .as_ref()
                        .map_or_else(|| "<none>".to_string(), borsa_core::Money::format);
                    let est = row
                        .estimate
                        .as_ref()
                        .map_or_else(|| "<none>".to_string(), borsa_core::Money::format);
                    println!(" - {}: {} vs {}", row.period, act, est);
                }
            }
        }
        Err(err) => println!("(earnings unavailable: {err})"),
    }

    // Income Statement
    println!("\n## Income Statement");
    match (inc_ann_res, inc_q_res) {
        (Ok(annual), Ok(quarterly)) => {
            if let Some(latest) = annual.first() {
                println!(
                    "Annual (latest {}): revenue={}, gross_profit={}, operating_income={}, net_income={}",
                    latest.period,
                    fmt_money(latest.total_revenue.as_ref()),
                    fmt_money(latest.gross_profit.as_ref()),
                    fmt_money(latest.operating_income.as_ref()),
                    fmt_money(latest.net_income.as_ref()),
                );
            }
            if let Some(latest) = quarterly.first() {
                println!(
                    "Quarterly (latest {}): revenue={}, gross_profit={}, operating_income={}, net_income={}",
                    latest.period,
                    fmt_money(latest.total_revenue.as_ref()),
                    fmt_money(latest.gross_profit.as_ref()),
                    fmt_money(latest.operating_income.as_ref()),
                    fmt_money(latest.net_income.as_ref()),
                );
            }
        }
        (a, q) => {
            if let Err(e) = a {
                println!("(annual income statement unavailable: {e})");
            }
            if let Err(e) = q {
                println!("(quarterly income statement unavailable: {e})");
            }
        }
    }

    // Balance Sheet
    println!("\n## Balance Sheet");
    match (bs_ann_res, bs_q_res) {
        (Ok(annual), Ok(quarterly)) => {
            if let Some(latest) = annual.first() {
                println!(
                    "Annual (latest {}): total_assets={}, total_liabilities={}, total_equity={}, cash={}, long_term_debt={}",
                    latest.period,
                    fmt_money(latest.total_assets.as_ref()),
                    fmt_money(latest.total_liabilities.as_ref()),
                    fmt_money(latest.total_equity.as_ref()),
                    fmt_money(latest.cash.as_ref()),
                    fmt_money(latest.long_term_debt.as_ref()),
                );
            }
            if let Some(latest) = quarterly.first() {
                println!(
                    "Quarterly (latest {}): total_assets={}, total_liabilities={}, total_equity={}, cash={}, long_term_debt={}",
                    latest.period,
                    fmt_money(latest.total_assets.as_ref()),
                    fmt_money(latest.total_liabilities.as_ref()),
                    fmt_money(latest.total_equity.as_ref()),
                    fmt_money(latest.cash.as_ref()),
                    fmt_money(latest.long_term_debt.as_ref()),
                );
            }
        }
        (a, q) => {
            if let Err(e) = a {
                println!("(annual balance sheet unavailable: {e})");
            }
            if let Err(e) = q {
                println!("(quarterly balance sheet unavailable: {e})");
            }
        }
    }

    // Cash Flow
    println!("\n## Cash Flow");
    match (cf_ann_res, cf_q_res) {
        (Ok(annual), Ok(quarterly)) => {
            if let Some(latest) = annual.first() {
                println!(
                    "Annual (latest {}): operating_cashflow={}, capex={}, free_cash_flow={}, net_income={}",
                    latest.period,
                    fmt_money(latest.operating_cashflow.as_ref()),
                    fmt_money(latest.capital_expenditures.as_ref()),
                    fmt_money(latest.free_cash_flow.as_ref()),
                    fmt_money(latest.net_income.as_ref()),
                );
            }
            if let Some(latest) = quarterly.first() {
                println!(
                    "Quarterly (latest {}): operating_cashflow={}, capex={}, free_cash_flow={}, net_income={}",
                    latest.period,
                    fmt_money(latest.operating_cashflow.as_ref()),
                    fmt_money(latest.capital_expenditures.as_ref()),
                    fmt_money(latest.free_cash_flow.as_ref()),
                    fmt_money(latest.net_income.as_ref()),
                );
            }
        }
        (a, q) => {
            if let Err(e) = a {
                println!("(annual cash flow unavailable: {e})");
            }
            if let Err(e) = q {
                println!("(quarterly cash flow unavailable: {e})");
            }
        }
    }

    // Calendar
    println!("\n## Calendar");
    match calendar_res {
        Ok(c) => {
            let next_earnings = c.earnings_dates.first().copied();
            println!(
                "Next Earnings: {}\nEx-Dividend: {}\nDividend Pay: {}",
                fmt_date(next_earnings),
                fmt_date(c.ex_dividend_date),
                fmt_date(c.dividend_payment_date)
            );
        }
        Err(err) => println!("(calendar unavailable: {err})"),
    }

    Ok(())
}
