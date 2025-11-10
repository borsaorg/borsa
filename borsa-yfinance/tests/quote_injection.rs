#![cfg(feature = "test-adapters")]

use borsa_core::Exchange;
use borsa_core::{AssetKind, Instrument, connector::QuoteProvider};
use borsa_yfinance::{YfConnector, adapter};

use std::sync::Arc;
use yfinance_rs as yf;

struct Combo {
    q: Arc<dyn adapter::YfQuotes>,
}
impl adapter::CloneArcAdapters for Combo {
    fn clone_arc_quotes(&self) -> Arc<dyn adapter::YfQuotes> {
        self.q.clone()
    }
}

#[tokio::test]
async fn quote_uses_injected_adapter() {
    // Fake quotes adapter returns a single quote for the requested symbol.
    let quotes = <dyn adapter::YfQuotes>::from_fn(|symbols| {
        assert_eq!(symbols, vec!["AAPL".to_string()]);
        Ok(vec![yf::core::Quote {
            instrument: Instrument::from_symbol("AAPL", AssetKind::Equity).unwrap(),
            shortname: None,
            price: Some(
                borsa_core::Money::from_canonical_str(
                    "123.45",
                    borsa_core::Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
            ),
            previous_close: Some(
                borsa_core::Money::from_canonical_str(
                    "120.00",
                    borsa_core::Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
            ),
            exchange: Some(borsa_core::Exchange::try_from_str("NasdaqGS").unwrap()),
            market_state: Some("CLOSED".parse::<borsa_core::MarketState>().unwrap()),
            day_volume: None,
        }])
    });

    let yf = YfConnector::from_adapter(&Combo { q: quotes });

    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid test instrument");
    let q = yf.quote(&inst).await.unwrap();

    let sym = match q.instrument.id() {
        borsa_core::IdentifierScheme::Security(sec) => sec.symbol.as_str(),
        borsa_core::IdentifierScheme::Prediction(_) => "<non-security>",
    };
    assert_eq!(sym, "AAPL");
    assert_eq!(
        q.price.as_ref().map(|m| m.amount().to_string()).as_deref(),
        Some("123.45")
    );
    assert_eq!(
        q.previous_close
            .as_ref()
            .map(|m| m.amount().to_string())
            .as_deref(),
        Some("120.00")
    );
}

#[test]
fn quote_injection_exchange() {
    let ex = Exchange::try_from_str("NasdaqGS").unwrap();
    assert_eq!(ex.code(), "NASDAQGS");
}
