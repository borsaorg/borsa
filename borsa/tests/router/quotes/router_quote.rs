use borsa::Borsa;
use borsa_core::{
    AssetKind, BorsaConnector, Exchange, Instrument, Quote, RoutingPolicyBuilder, Symbol,
};

use crate::helpers::{MockConnector, m_quote, usd, X};
use std::sync::Arc;

#[tokio::test]
async fn quote_fallback_first_success() {
    let c1 = m_quote("a", 0.0);
    let c2 = m_quote("b", 42.0);

    let borsa = Borsa::builder()
        .with_connector(c1)
        .with_connector(c2)
        .build()
        .unwrap();

    let inst = crate::helpers::instrument(&X, AssetKind::Equity);
    let q = borsa.quote(&inst).await.unwrap();
    assert_eq!(
        q.price.unwrap().amount(),
        rust_decimal::Decimal::from_f64_retain(0.0).unwrap()
    );
}

#[tokio::test]
async fn quote_respects_priority_override() {
    let c1 = m_quote("low", 10.0);
    let c2 = m_quote("high", 99.0);

    let policy = RoutingPolicyBuilder::new()
        .providers_for_symbol(&X, &[c2.key(), c1.key()])
        .build();

    let borsa = Borsa::builder()
        .with_connector(c1.clone())
        .with_connector(c2.clone())
        .routing_policy(policy)
        .build()
        .unwrap();

    let inst = crate::helpers::instrument(&X, AssetKind::Equity);
    let q = borsa.quote(&inst).await.unwrap();
    assert_eq!(
        q.price.unwrap().amount(),
        rust_decimal::Decimal::from_f64_retain(99.0).unwrap()
    );
}

fn instrument_with_exchange(symbol: &str, exchange: Exchange, kind: AssetKind) -> Instrument {
    Instrument::from_symbol_and_exchange(symbol, exchange, kind).unwrap()
}

fn connector_with_quote(
    name: &'static str,
    price: &str,
    exchange: Option<Exchange>,
) -> Arc<MockConnector> {
    MockConnector::builder()
        .name(name)
        .returns_quote_ok(Quote {
            symbol: Symbol::new("X").unwrap(),
            shortname: None,
            price: Some(usd(price)),
            previous_close: None,
            exchange,
            market_state: None,
            day_volume: None,
        })
        .build()
}

#[tokio::test]
async fn quote_falls_back_when_exchange_mismatches() {
    let nyse = Exchange::try_from_str("NYSE").unwrap();
    let nasdaq = Exchange::try_from_str("NASDAQ").unwrap();

    let mismatch = connector_with_quote("mismatch", "10.0", Some(nasdaq));
    let matching = connector_with_quote("match", "99.0", Some(nyse.clone()));

    let borsa = Borsa::builder()
        .with_connector(mismatch.clone())
        .with_connector(matching.clone())
        .build()
        .unwrap();

    let inst = instrument_with_exchange("X", nyse.clone(), AssetKind::Equity);
    let q = borsa.quote(&inst).await.unwrap();

    assert_eq!(q.price.unwrap().amount().to_string(), "99.0");
    assert_eq!(q.exchange, Some(nyse));
}

#[tokio::test]
async fn quote_without_exchange_is_accepted_when_not_provided() {
    let nyse = Exchange::try_from_str("NYSE").unwrap();
    let no_exchange = connector_with_quote("no-ex", "55.5", None);

    let borsa = Borsa::builder()
        .with_connector(no_exchange.clone())
        .build()
        .unwrap();

    let inst = instrument_with_exchange("X", nyse, AssetKind::Equity);
    let q = borsa.quote(&inst).await.unwrap();

    assert_eq!(q.price.unwrap().amount().to_string(), "55.5");
    assert!(q.exchange.is_none());
}

#[tokio::test]
async fn quote_respects_exchange_override_in_provider_policy() {
    let ex = Exchange::try_from_str("NASDAQ").unwrap();
    let ex_for_rule = ex.clone();

    let low = m_quote("low", 10.0);
    let high = m_quote("high", 99.0);

    let policy = RoutingPolicyBuilder::new()
        .providers_for_exchange(ex_for_rule, &[high.key(), low.key()])
        .build();

    let borsa = Borsa::builder()
        .with_connector(low.clone())
        .with_connector(high.clone())
        .routing_policy(policy)
        .build()
        .unwrap();

    let inst = Instrument::from_symbol_and_exchange("X", ex, AssetKind::Equity).unwrap();
    let q = borsa.quote(&inst).await.unwrap();
    assert_eq!(
        q.price.unwrap().amount(),
        rust_decimal::Decimal::from_f64_retain(99.0).unwrap()
    );
}

#[tokio::test]
async fn quote_strict_rule_blocks_fallback() {
    // Only provider returns NotFound; strict prevents fallback to other providers.
    let only = MockConnector::builder()
        .name("only")
        .with_quote_fn(|_i| Err(borsa_core::BorsaError::not_found("quote for X")))
        .build();
    let other = m_quote("other", 123.45);

    let x = Symbol::new("X").expect("valid symbol");
    let policy = RoutingPolicyBuilder::new()
        .providers_rule(
            borsa_core::Selector {
                symbol: Some(x),
                kind: Some(AssetKind::Equity),
                exchange: None,
            },
            &[only.key()],
            true,
        )
        .build();

    let borsa = Borsa::builder()
        .with_connector(only.clone())
        .with_connector(other.clone())
        .routing_policy(policy)
        .build()
        .unwrap();

    let inst = crate::helpers::instrument(&X, AssetKind::Equity);
    let err = borsa.quote(&inst).await.expect_err("should not fallback");
    assert!(matches!(err, borsa_core::BorsaError::NotFound { .. }));
}
