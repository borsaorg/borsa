use borsa::Borsa;
use borsa_core::{AssetKind, Instrument};
use borsa_examples::common::get_connector;

// Helper to format a Unix timestamp into a readable date string.
fn format_date(ts: Option<chrono::DateTime<chrono::Utc>>) -> String {
    ts.map_or_else(
        || "Not scheduled".to_string(),
        |dt| dt.format("%A, %B %e, %Y").to_string(),
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Setup Borsa with selected connector (mock in CI when BORSA_EXAMPLES_USE_MOCK is set).
    let connector = get_connector();
    let borsa = Borsa::builder().with_connector(connector).build()?;

    // 2. Define the instrument.
    let instrument =
        Instrument::from_symbol("JPM", AssetKind::Equity).expect("valid instrument symbol");
    println!("Fetching event calendar for {}...", instrument.symbol());

    // 3. Fetch the calendar data.
    let calendar = borsa.calendar(&instrument).await?;

    // 4. Print the results.
    println!("\n## Event Calendar for {}", instrument.symbol());
    if let Some(next_earnings) = calendar.earnings_dates.first() {
        println!(
            "- Next Earnings Date: {}",
            format_date(Some(*next_earnings))
        );
    } else {
        println!("- Next Earnings Date: Not scheduled");
    }

    println!(
        "- Ex-Dividend Date:   {}",
        format_date(calendar.ex_dividend_date)
    );
    println!(
        "- Dividend Pay Date:  {}",
        format_date(calendar.dividend_payment_date)
    );

    Ok(())
}
