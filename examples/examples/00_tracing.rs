use borsa::Borsa;
use borsa_core::{AssetKind, HistoryRequest, Instrument, Interval, Range, SearchRequest};
use borsa_examples::common::get_connector;
use tracing_subscriber::fmt::format::FmtSpan;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize a human-friendly tracing subscriber with env-based filtering.
    // Suggested: RUST_LOG=info,borsa=trace,borsa_yfinance=trace
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .with_span_events(FmtSpan::ENTER | FmtSpan::EXIT)
        .try_init();

    // Create connector (mock in CI when BORSA_EXAMPLES_USE_MOCK is set) and build router
    let connector = get_connector();
    let borsa = Borsa::builder().with_connector(connector).build()?;

    // Define an instrument
    let aapl = Instrument::from_symbol("AAPL", AssetKind::Equity)?;

    // Quote
    let _ = borsa.quote(&aapl).await?;

    // History (6 months daily)
    let req = HistoryRequest::try_from_range(Range::M6, Interval::D1)?;
    let _ = borsa.history(&aapl, req).await?;

    // Search
    let req = SearchRequest::new("Apple")?;
    let _ = borsa.search(req).await?;

    Ok(())
}
