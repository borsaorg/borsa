use std::sync::Arc;
use std::time::Duration;

use borsa::Borsa;
use borsa_core::{AssetKind, BorsaConnector, Instrument};
use borsa_mock::MockConnector;
use borsa_yfinance::YfConnector;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1) Build a rate-limited yfinance connector with a tiny window for demo purposes: 1 call per 3s
    let yf: Arc<dyn BorsaConnector> = YfConnector::rate_limited()
        .quota_limit(1)
        .quota_window(Duration::from_secs(3))
        .build();

    println!("Outer connector identity: {}", yf.name());

    let aapl = Instrument::from_symbol("AAPL", AssetKind::Equity)?;

    // 2) Build a router with YF first, Mock second. When YF is rate-limited, router should fall back to Mock.
    let mock: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());
    let borsa = Borsa::builder()
        .with_connector(Arc::clone(&yf))
        .with_connector(Arc::clone(&mock))
        .build()?;

    // 3) First router call should succeed from YF (quota available)
    let q1 = borsa.quote(&aapl).await?;
    if let Some(p) = &q1.price {
        println!("First router call: price={}", p.format());
    }

    // 4) Immediate second router call should hit YF quota and fall back to Mock
    let q2 = borsa.quote(&aapl).await?;
    let from_mock = q2.price.as_ref().map(borsa_core::Money::format)
        == Some("190.00 USD".to_string())
        && q2.previous_close.as_ref().map(borsa_core::Money::format)
            == Some("188.00 USD".to_string());
    println!(
        "Second router call: price={} (fallback observed: {})",
        q2.price
            .as_ref()
            .map_or_else(|| "<none>".to_string(), borsa_core::Money::format),
        from_mock
    );

    // 5) Wait for the YF quota window to reset, then the router should use YF again
    println!("Sleeping 3s to allow YF quota window to reset...");
    tokio::time::sleep(Duration::from_secs(3)).await;
    let q3 = borsa.quote(&aapl).await?;
    let likely_yf = !(q3.price.as_ref().map(borsa_core::Money::format)
        == Some("190.00 USD".to_string())
        && q3.previous_close.as_ref().map(borsa_core::Money::format)
            == Some("188.00 USD".to_string()));
    println!(
        "Third router call after reset: price={} (likely YF again: {})",
        q3.price
            .as_ref()
            .map_or_else(|| "<none>".to_string(), borsa_core::Money::format),
        likely_yf
    );

    Ok(())
}
