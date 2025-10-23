use async_trait::async_trait;
use borsa::Borsa;
use borsa_core::{
    AssetKind, BorsaConnector, BorsaError, Currency, Instrument, Money, Quote,
    RoutingPolicyBuilder, connector::QuoteProvider,
};
use std::sync::Arc;
use tokio::time::{Duration, sleep};

// A fast connector that provides a "less accurate" price.
struct FastConnector;
#[async_trait]
impl BorsaConnector for FastConnector {
    fn name(&self) -> &'static str {
        "fast-but-inaccurate"
    }

    fn as_quote_provider(&self) -> Option<&dyn QuoteProvider> {
        Some(self as &dyn QuoteProvider)
    }

    fn supports_kind(&self, _kind: AssetKind) -> bool {
        true
    }
}

// A slow connector that provides a "more accurate" price.
struct SlowConnector;
#[async_trait]
impl BorsaConnector for SlowConnector {
    fn name(&self) -> &'static str {
        "slow-but-accurate"
    }

    fn as_quote_provider(&self) -> Option<&dyn QuoteProvider> {
        Some(self as &dyn QuoteProvider)
    }

    fn supports_kind(&self, _kind: AssetKind) -> bool {
        true
    }
}

#[async_trait]
impl QuoteProvider for FastConnector {
    async fn quote(&self, i: &Instrument) -> Result<Quote, BorsaError> {
        println!("-> FastConnector responding for {}", i.symbol());
        Ok(Quote {
            symbol: i.symbol().clone(),
            shortname: None,
            price: Some(
                Money::from_canonical_str("100.0", Currency::Iso(borsa_core::IsoCurrency::USD))
                    .unwrap(),
            ),
            previous_close: None,
            exchange: None,
            market_state: None,
        })
    }
}

#[async_trait]
impl QuoteProvider for SlowConnector {
    async fn quote(&self, i: &Instrument) -> Result<Quote, BorsaError> {
        sleep(Duration::from_millis(50)).await; // Simulate network latency
        println!("-> SlowConnector responding for {}", i.symbol());
        Ok(Quote {
            symbol: i.symbol().clone(),
            shortname: None,
            price: Some(
                Money::from_canonical_str("999.99", Currency::Iso(borsa_core::IsoCurrency::USD))
                    .unwrap(),
            ),
            previous_close: None,
            exchange: None,
            market_state: None,
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Setup our mock connectors.
    let fast_conn = Arc::new(FastConnector);
    let slow_conn = Arc::new(SlowConnector);

    // 2. Build Borsa with a default priority and a per-symbol override.
    let policy = RoutingPolicyBuilder::new()
        .providers_for_kind(AssetKind::Equity, &[fast_conn.key(), slow_conn.key()])
        .providers_for_symbol("SPECIAL", &[slow_conn.key(), fast_conn.key()])
        .build();
    let borsa = Borsa::builder()
        .with_connector(fast_conn.clone())
        .with_connector(slow_conn.clone())
        .routing_policy(policy)
        .build()?;

    // --- SCENARIO 1: Fetch a normal symbol ---
    println!("# Fetching quote for a normal symbol ('NORMAL')...");
    let normal_inst =
        Instrument::from_symbol("NORMAL", AssetKind::Equity).expect("valid instrument symbol");
    let normal_quote = borsa.quote(&normal_inst).await?;
    println!(
        "\nResult for 'NORMAL': ${:.2} (from the fast connector, as per default priority)\n",
        normal_quote
            .price
            .as_ref()
            .map(borsa_core::Money::amount)
            .unwrap_or_default()
    );

    // --- SCENARIO 2: Fetch the special symbol ---
    println!("# Fetching quote for symbol 'SPECIAL' with a priority override...");
    let special_inst =
        Instrument::from_symbol("SPECIAL", AssetKind::Equity).expect("valid instrument symbol");
    let special_quote = borsa.quote(&special_inst).await?;
    println!(
        "\nResult for 'SPECIAL': ${:.2} (from the slow connector, due to the per-symbol override)\n",
        special_quote
            .price
            .as_ref()
            .map(borsa_core::Money::amount)
            .unwrap_or_default()
    );

    Ok(())
}
