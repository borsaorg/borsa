use crate::helpers::{AAPL, MSFT, MockConnector, quote_fixture};
use borsa::Borsa;
use borsa_core::{AssetKind, BorsaConnector, RoutingPolicyBuilder};
use rust_decimal::Decimal;
use std::collections::HashMap;
#[tokio::test]
async fn router_quotes_multi_groups_by_provider() {
    // Provider 'a' is preferred for AAPL and returns a specific price.
    let conn_a = MockConnector::builder()
        .name("a")
        .returns_quote_ok(quote_fixture(&AAPL, "100.0"))
        .build();

    // Provider 'b' is preferred for MSFT and returns a different price.
    let conn_b = MockConnector::builder()
        .name("b")
        .returns_quote_ok(quote_fixture(&MSFT, "200.0"))
        .build();

    let policy = RoutingPolicyBuilder::new()
        .providers_for_symbol(&AAPL, &[conn_a.key(), conn_b.key()])
        .providers_for_symbol(&MSFT, &[conn_b.key(), conn_a.key()])
        .build();

    let borsa = Borsa::builder()
        .with_connector(conn_a.clone())
        .with_connector(conn_b.clone())
        .routing_policy(policy)
        .build()
        .unwrap();

    let instruments = &[
        crate::helpers::instrument(&AAPL, AssetKind::Equity),
        crate::helpers::instrument(&MSFT, AssetKind::Equity),
    ];

    let (quotes, errs) = borsa.quotes(instruments).await.unwrap();
    assert!(errs.is_empty());

    assert_eq!(quotes.len(), 2);

    let by_symbol: HashMap<borsa_core::Symbol, &borsa_core::Quote> = quotes
        .iter()
        .map(|q| {
            let sym = match q.instrument.id() {
                borsa_core::IdentifierScheme::Security(sec) => sec.symbol.clone(),
                borsa_core::IdentifierScheme::Prediction(_) => {
                    panic!("unexpected non-security")
                }
            };
            (sym, q)
        })
        .collect();

    assert_eq!(
        by_symbol
            .get(&AAPL)
            .unwrap()
            .price
            .as_ref()
            .unwrap()
            .amount(),
        Decimal::from(100u8)
    );
    assert_eq!(
        by_symbol
            .get(&MSFT)
            .unwrap()
            .price
            .as_ref()
            .unwrap()
            .amount(),
        Decimal::from(200u8)
    );
}
