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
        if let Some(price) = &q.price {
            println!("{}: {}", q.symbol.as_str(), price.format());
        } else {
            println!("{}: <no price>", q.symbol.as_str());
        }
    }

    if !failures.is_empty() {
        eprintln!("Failures:");
        for (inst, err) in failures {
            eprintln!("- {} -> {}", inst.symbol().as_str(), err);
        }
    }

    Ok(())
}
