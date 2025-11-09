mod common;
use borsa::Borsa;
use borsa_core::{AssetKind, Instrument};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use YfConnector by default; CI may set BORSA_EXAMPLES_USE_MOCK to use a mock.
    let connector = common::get_connector();
    let borsa = Borsa::builder().with_connector(connector).build()?;

    let aapl = Instrument::from_symbol("AAPL", AssetKind::Equity)?;
    let sym_str = match aapl.id() {
        borsa_core::IdentifierScheme::Security(sec) => sec.symbol.as_str(),
        borsa_core::IdentifierScheme::Prediction(_) => "<non-security>",
    };

    println!("Starting stream for {sym_str}... (running for ~5s)");
    let (handle, mut rx) = match borsa.stream_quotes(std::slice::from_ref(&aapl)).await {
        Ok(parts) => parts,
        Err(e) => {
            eprintln!("stream not supported or failed to start: {e}");
            return Ok(());
        }
    };

    let printer = tokio::spawn(async move {
        let mut count = 0usize;
        while let Some(update) = rx.recv().await {
            println!("update: {update:?}");
            count += 1;
            if count >= 20 {
                break;
            }
        }
        count
    });

    tokio::time::sleep(Duration::from_secs(5)).await;
    handle.stop().await;
    let _ = printer.await;
    println!("stream stopped");

    Ok(())
}
