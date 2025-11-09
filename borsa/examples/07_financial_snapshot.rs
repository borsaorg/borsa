mod common;
use borsa::Borsa;
use borsa_core::{AssetKind, Instrument, Profile};
use common::get_connector;
use rust_decimal::Decimal;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Setup Borsa with selected connector (mock in CI when BORSA_EXAMPLES_USE_MOCK is set).
    let connector = get_connector();
    let borsa = Borsa::builder().with_connector(connector).build()?;

    // 2. Define the instrument.
    let instrument =
        Instrument::from_symbol("META", AssetKind::Equity).expect("valid instrument symbol");
    let sym_str = match instrument.id() {
        borsa_core::IdentifierScheme::Security(sec) => sec.symbol.as_str(),
        borsa_core::IdentifierScheme::Prediction(_) => "<non-security>",
    };
    println!("Fetching financial snapshot for {sym_str}...");

    // 3. Fetch quote, profile, and earnings data concurrently.
    let (quote_res, profile_res, earnings_res) = tokio::join!(
        borsa.quote(&instrument),
        borsa.profile(&instrument),
        borsa.earnings(&instrument)
    );

    println!("\n========================================");
    println!("Financial Snapshot for {sym_str}");
    println!("========================================");

    // 4. Print the quote information.
    if let Ok(quote) = quote_res {
        let price = quote
            .price
            .as_ref()
            .map(borsa_core::Money::amount)
            .unwrap_or_default();
        let prev_close = quote
            .previous_close
            .as_ref()
            .map(borsa_core::Money::amount)
            .unwrap_or_default();
        let change = price - prev_close;
        let change_pct = if prev_close == Decimal::ZERO {
            Decimal::ZERO
        } else {
            (change / prev_close) * Decimal::from(100u8)
        };
        println!("\n## Market Quote");
        println!("Last Price: ${price:.2} ({change:+.2} / {change_pct:+.2}%)");
        println!(
            "Exchange:   {}",
            quote.exchange.map(|e| e.to_string()).unwrap_or_default()
        );
    }

    // 5. Print the profile information.
    if let Ok(Profile::Company(profile)) = profile_res {
        println!("\n## Company Profile");
        println!("Name:     {}", profile.name);
        println!("Sector:   {}", profile.sector.unwrap_or_default());
        println!("Industry: {}", profile.industry.unwrap_or_default());
    }

    // 6. Print the latest yearly earnings.
    if let Ok(earnings) = earnings_res
        && let Some(latest_year) = earnings.yearly.last()
    {
        println!("\n## Latest Annual Earnings ({})", latest_year.year);
        if let Some(rev) = latest_year.revenue.as_ref() {
            println!(
                "Revenue:  ${:.2}B",
                rev.amount() / Decimal::from(1_000_000_000u64)
            );
        }
        if let Some(earn) = latest_year.earnings.as_ref() {
            println!(
                "Earnings: ${:.2}B",
                earn.amount() / Decimal::from(1_000_000_000u64)
            );
        }
    }

    Ok(())
}
