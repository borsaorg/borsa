use borsa::Borsa;
use borsa_core::{AssetKind, BorsaConnector, RoutingPolicyBuilder};

use crate::helpers::m_quote;

#[tokio::test]
async fn last_defined_rule_wins_on_equal_specificity() {
    let a = m_quote("a", 10.0);
    let b = m_quote("b", 99.0);

    // Define two symbol-level rules with equal specificity for the same symbol.
    // The latter should take precedence.
    let policy = RoutingPolicyBuilder::new()
        .providers_for_symbol("X", &[a.key(), b.key()])
        .providers_for_symbol("X", &[b.key(), a.key()])
        .build();

    let borsa = Borsa::builder()
        .with_connector(a.clone())
        .with_connector(b.clone())
        .routing_policy(policy)
        .build()
        .unwrap();

    let inst = crate::helpers::instrument("X", AssetKind::Equity);
    let q = borsa.quote(&inst).await.unwrap();
    // Expect provider 'b' to be first due to last-defined rule taking precedence.
    assert_eq!(q.price.unwrap().amount().to_string(), "99.0");
}

#[tokio::test]
async fn symbol_rule_beats_exchange_rule_even_if_defined_first() {
    let sym = m_quote("sym", 42.0);
    let ex = m_quote("ex", 7.0);

    let nasdaq = borsa_core::Exchange::try_from_str("NASDAQ").unwrap();

    let policy = RoutingPolicyBuilder::new()
        .providers_for_symbol("X", &[sym.key(), ex.key()])
        .providers_for_exchange(nasdaq.clone(), &[ex.key(), sym.key()])
        .build();

    let borsa = Borsa::builder()
        .with_connector(sym.clone())
        .with_connector(ex.clone())
        .routing_policy(policy)
        .build()
        .unwrap();

    let inst = borsa_core::Instrument::from_symbol_and_exchange("X", nasdaq, AssetKind::Equity)
        .unwrap();
    let q = borsa.quote(&inst).await.unwrap();
    assert_eq!(q.price.unwrap().amount().to_string(), "42.0");
}


