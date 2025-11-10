mod common;
use borsa::Borsa;
use borsa_core::{AssetKind, Instrument};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let connector = common::get_connector();
    let borsa = Borsa::builder().with_connector(connector).build()?;

    let instruments = [
        Instrument::from_symbol("AAPL", AssetKind::Equity)?,
        Instrument::from_symbol("GOOGL", AssetKind::Equity)?,
        Instrument::from_symbol("BTC-USD", AssetKind::Crypto)?,
    ];

    let (quotes, failures) = borsa.quotes(&instruments).await?;
    for q in quotes {
        let sym = match q.instrument.id() {
            borsa_core::IdentifierScheme::Security(sec) => sec.symbol.as_str(),
            borsa_core::IdentifierScheme::Prediction(_) => "<non-security>",
        };
        if let Some(price) = &q.price {
            println!("{}: {}", sym, price.format());
        } else {
            println!("{sym}: <no price>");
        }
    }

    if !failures.is_empty() {
        eprintln!("Failures:");
        for (inst, err) in failures {
            let sym_str = match inst.id() {
                borsa_core::IdentifierScheme::Security(sec) => sec.symbol.as_str(),
                borsa_core::IdentifierScheme::Prediction(_) => "<non-security>",
            };
            eprintln!("- {sym_str} -> {err}");
        }
    }

    Ok(())
}
