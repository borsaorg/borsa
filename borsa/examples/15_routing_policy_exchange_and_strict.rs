use async_trait::async_trait;
use borsa::Borsa;
use borsa_core::{
    AssetKind, BorsaConnector, BorsaError, Exchange, Instrument, Quote, RoutingPolicyBuilder,
    connector::QuoteProvider,
};
use std::sync::Arc;
use tokio::time::{Duration, sleep};

struct FastConnector;
struct SlowConnector;

#[async_trait]
impl BorsaConnector for FastConnector {
    fn name(&self) -> &'static str {
        "fast"
    }
    fn as_quote_provider(&self) -> Option<&dyn QuoteProvider> {
        Some(self as &dyn QuoteProvider)
    }
    fn supports_kind(&self, _kind: AssetKind) -> bool {
        true
    }
}

#[async_trait]
impl BorsaConnector for SlowConnector {
    fn name(&self) -> &'static str {
        "slow"
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
        let sym = match i.id() {
            borsa_core::IdentifierScheme::Security(sec) => sec.symbol.clone(),
            borsa_core::IdentifierScheme::Prediction(_) => {
                return Err(BorsaError::unsupported(
                    "instrument scheme (example/security-only)",
                ));
            }
        };
        Ok(Quote {
            symbol: sym,
            shortname: None,
            price: Some(
                borsa_core::Money::from_canonical_str(
                    "100.00",
                    borsa_core::Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
            ),
            previous_close: None,
            exchange: None,
            market_state: None,
            day_volume: None,
        })
    }
}

#[async_trait]
impl QuoteProvider for SlowConnector {
    async fn quote(&self, i: &Instrument) -> Result<Quote, BorsaError> {
        sleep(Duration::from_millis(25)).await;
        let sym = match i.id() {
            borsa_core::IdentifierScheme::Security(sec) => sec.symbol.clone(),
            borsa_core::IdentifierScheme::Prediction(_) => {
                return Err(BorsaError::unsupported(
                    "instrument scheme (example/security-only)",
                ));
            }
        };
        Ok(Quote {
            symbol: sym,
            shortname: None,
            price: Some(
                borsa_core::Money::from_canonical_str(
                    "999.00",
                    borsa_core::Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
            ),
            previous_close: None,
            exchange: None,
            market_state: None,
            day_volume: None,
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fast = Arc::new(FastConnector);
    let slow = Arc::new(SlowConnector);

    let nasdaq = Exchange::try_from_str("NASDAQ").unwrap();

    // Global default: prefer fast, but prefer slow for NASDAQ symbols.
    // Additionally, make Crypto strict to only use slow (no fallback to fast).
    let policy = RoutingPolicyBuilder::new()
        .providers_global(&[fast.key(), slow.key()])
        .providers_for_exchange(nasdaq.clone(), &[slow.key(), fast.key()])
        .providers_rule(
            borsa_core::Selector {
                symbol: None,
                kind: Some(AssetKind::Crypto),
                exchange: None,
            },
            &[slow.key()],
            true, // strict: no fallback beyond listed providers
        )
        .build();

    let borsa = Borsa::builder()
        .with_connector(fast.clone())
        .with_connector(slow.clone())
        .routing_policy(policy)
        .build()?;

    // 1) Equity with NASDAQ exchange → slow wins due to exchange override
    let aapl = Instrument::from_symbol_and_exchange("AAPL", nasdaq, AssetKind::Equity)?;
    let q1 = borsa.quote(&aapl).await?;
    println!("AAPL@NASDAQ -> ${}", q1.price.unwrap().format());

    // 2) Equity without exchange → global default (fast first)
    let msft = Instrument::from_symbol("MSFT", AssetKind::Equity)?;
    let q2 = borsa.quote(&msft).await?;
    println!("MSFT -> ${}", q2.price.unwrap().format());

    // 3) Crypto strict rule → only slow provider is attempted
    let btc = Instrument::from_symbol("BTC-USD", AssetKind::Crypto)?;
    let q3 = borsa.quote(&btc).await?;
    println!("BTC-USD (strict) -> ${}", q3.price.unwrap().format());

    // Console output should show slow for AAPL and BTC-USD; fast for MSFT.
    Ok(())
}
