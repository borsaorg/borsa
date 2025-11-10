#![cfg(feature = "test-adapters")]

use std::sync::Arc;

use async_trait::async_trait;
use borsa_core::{
    AssetKind, Currency, Instrument, Money,
    connector::{OptionChainProvider, OptionsExpirationsProvider},
};
use borsa_yfinance::{YfConnector, adapter};
use chrono::TimeZone;
use yfinance_rs as yf;

struct StubOptions;
#[async_trait]
impl adapter::YfOptions for StubOptions {
    async fn expirations(&self, symbol: &str) -> Result<Vec<i64>, borsa_core::BorsaError> {
        assert_eq!(symbol, "AAPL");
        Ok(vec![1_725_813_600, 1_726_400_000])
    }
    async fn chain(
        &self,
        symbol: &str,
        date: Option<i64>,
    ) -> Result<yf::ticker::OptionChain, borsa_core::BorsaError> {
        assert_eq!(symbol, "AAPL");
        assert_eq!(date, Some(1_725_813_600));
        Ok(yf::ticker::OptionChain {
            calls: vec![yf::ticker::OptionContract {
                instrument: Instrument::from_symbol("AAPL250620C00100000", AssetKind::Equity)
                    .unwrap(),
                strike: Money::from_canonical_str(
                    "100",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                price: Some(
                    Money::from_canonical_str("1.23", Currency::Iso(borsa_core::IsoCurrency::USD))
                        .unwrap(),
                ),
                bid: Some(
                    Money::from_canonical_str("1.20", Currency::Iso(borsa_core::IsoCurrency::USD))
                        .unwrap(),
                ),
                ask: Some(
                    Money::from_canonical_str("1.30", Currency::Iso(borsa_core::IsoCurrency::USD))
                        .unwrap(),
                ),
                volume: Some(123),
                open_interest: Some(456),
                implied_volatility: Some(0.35),
                in_the_money: false,
                expiration_at: Some(chrono::Utc.timestamp_opt(1_725_813_600, 0).unwrap()),
                expiration_date: chrono::NaiveDate::from_ymd_opt(2024, 6, 25).unwrap(),
                greeks: None,
                last_trade_at: None,
            }],
            puts: vec![yf::ticker::OptionContract {
                instrument: Instrument::from_symbol("AAPL250620P00100000", AssetKind::Equity)
                    .unwrap(),
                strike: Money::from_canonical_str(
                    "100",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                price: Some(
                    Money::from_canonical_str("0.95", Currency::Iso(borsa_core::IsoCurrency::USD))
                        .unwrap(),
                ),
                bid: Some(
                    Money::from_canonical_str("0.90", Currency::Iso(borsa_core::IsoCurrency::USD))
                        .unwrap(),
                ),
                ask: Some(
                    Money::from_canonical_str("1.00", Currency::Iso(borsa_core::IsoCurrency::USD))
                        .unwrap(),
                ),
                volume: Some(99),
                open_interest: Some(321),
                implied_volatility: Some(0.37),
                in_the_money: true,
                expiration_at: Some(chrono::Utc.timestamp_opt(1_725_813_600, 0).unwrap()),
                expiration_date: chrono::NaiveDate::from_ymd_opt(2024, 6, 25).unwrap(),
                greeks: None,
                last_trade_at: None,
            }],
        })
    }
}

struct Combo {
    o: Arc<dyn adapter::YfOptions>,
}
impl adapter::CloneArcAdapters for Combo {
    fn clone_arc_history(&self) -> Arc<dyn adapter::YfHistory> {
        Arc::new(adapter::RealAdapter::new_default()) as Arc<_>
    }
    fn clone_arc_quotes(&self) -> Arc<dyn adapter::YfQuotes> {
        Arc::new(adapter::RealAdapter::new_default()) as Arc<_>
    }
    fn clone_arc_search(&self) -> Arc<dyn adapter::YfSearch> {
        Arc::new(adapter::RealAdapter::new_default()) as Arc<_>
    }
    fn clone_arc_options(&self) -> Arc<dyn adapter::YfOptions> {
        self.o.clone()
    }
    // If your trait includes fundamentals/profile with defaults, you can omit them here
}

#[tokio::test]
async fn options_injection_expirations_and_chain_map_correctly() {
    let yf = YfConnector::from_adapter(&Combo {
        o: Arc::new(StubOptions),
    });
    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid test instrument");

    // Expirations
    let exps = yf.options_expirations(&inst).await.unwrap();
    assert_eq!(exps, vec![1_725_813_600, 1_726_400_000]);

    // Chain
    let ch = yf.option_chain(&inst, Some(1_725_813_600)).await.unwrap();

    assert_eq!(ch.calls.len(), 1);
    assert_eq!(ch.puts.len(), 1);
    let call_symbol = match ch.calls[0].instrument.id() {
        borsa_core::IdentifierScheme::Security(sec) => sec.symbol.as_str(),
        borsa_core::IdentifierScheme::Prediction(_) => "<non-security>",
    };
    assert_eq!(call_symbol, "AAPL250620C00100000");
    assert!(ch.puts[0].in_the_money);
}
