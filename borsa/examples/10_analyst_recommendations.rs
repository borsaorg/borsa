mod common;
use borsa::Borsa;
use borsa_core::{AssetKind, Instrument};
use common::get_connector;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Setup Borsa with selected connector (mock in CI when BORSA_EXAMPLES_USE_MOCK is set).
    let connector = get_connector();
    let borsa = Borsa::builder().with_connector(connector).build()?;

    // 2. Define the instrument.
    let instrument =
        Instrument::from_symbol("MSFT", AssetKind::Equity).expect("valid instrument symbol");
    println!("Fetching analyst actions for {}...", instrument.symbol());

    // 3. Fetch recommendations and upgrade/downgrade history concurrently.
    let (recs_res, history_res) = tokio::join!(
        borsa.recommendations(&instrument),
        borsa.upgrades_downgrades(&instrument)
    );

    // 4. Print the latest recommendation summary.
    if let Ok(recommendations) = recs_res
        && let Some(latest) = recommendations.first()
    {
        println!("\n## Current Analyst Consensus (Period: {})", latest.period);
        println!("- Strong Buy: {:?}", latest.strong_buy);
        println!("- Buy:        {:?}", latest.buy);
        println!("- Hold:       {:?}", latest.hold);
        println!("- Sell:       {:?}", latest.sell);
        println!("- Strong Sell:{:?}", latest.strong_sell);
    }

    // 5. Print the last 5 rating changes.
    if let Ok(mut history) = history_res {
        println!("\n## Recent Rating Changes");
        println!(
            "{:<12} | {:<25} | {:<12} -> {:<12}",
            "Date", "Firm", "From", "To"
        );
        println!("{:-<13}|{:-<27}|{:-<27}", "", "", "");

        // The list is already sorted oldest to newest, so we reverse it for display.
        history.reverse();
        for event in history.iter().take(5) {
            let date = event.ts.format("%Y-%m-%d").to_string();
            println!(
                "{:<12} | {:<25} | {:?} -> {:?}",
                date,
                event.firm.as_deref().unwrap_or("N/A"),
                event.from_grade,
                event.to_grade,
            );
        }
    }

    Ok(())
}
