mod common;
use borsa::Borsa;
use borsa_core::{AssetKind, Instrument, Interval, Range};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let connector = common::get_connector();
    let borsa = Borsa::builder().with_connector(connector).build()?;

    let instruments = [
        Instrument::from_symbol("AAPL", AssetKind::Equity)?,
        Instrument::from_symbol("MSFT", AssetKind::Equity)?,
    ];

    let report = borsa
        .download()
        .instruments(&instruments)?
        .range(Range::M6)
        .interval(Interval::D1)
        .run()
        .await?;

    if let Some(resp) = report.response {
        for entry in resp.entries {
            let sym_str = match entry.instrument.id() {
                borsa_core::IdentifierScheme::Security(sec) => sec.symbol.as_str(),
                borsa_core::IdentifierScheme::Prediction(_) => "<non-security>",
            };
            println!("{}: {} candles", sym_str, entry.history.candles.len());
        }
    } else {
        eprintln!("no data returned");
    }

    if !report.warnings.is_empty() {
        eprintln!("warnings:");
        for w in report.warnings {
            eprintln!("- {w}");
        }
    }

    Ok(())
}
