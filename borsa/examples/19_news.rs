mod common;
use borsa::Borsa;
use borsa_core::{AssetKind, Instrument, NewsRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let connector = common::get_connector();
    let borsa = Borsa::builder().with_connector(connector).build()?;

    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity)?;
    let req = NewsRequest {
        count: 10,
        ..Default::default()
    };

    let sym_str = match inst.id() {
        borsa_core::IdentifierScheme::Security(sec) => sec.symbol.as_str(),
        borsa_core::IdentifierScheme::Prediction(_) => "<non-security>",
    };
    println!("Fetching news for {sym_str}...");
    let articles = borsa.news(&inst, req).await?;
    for a in articles.iter().take(5) {
        println!("{} — {}", a.title, a.publisher.as_deref().unwrap_or(""));
    }
    Ok(())
}
