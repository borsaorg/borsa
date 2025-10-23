use crate::helpers::{AAPL, usd};
use borsa_core::{AssetKind, BorsaConnector, QuoteUpdate, RoutingPolicyBuilder};
use chrono::TimeZone;

use crate::helpers::MockConnector;

#[tokio::test]
async fn stream_quotes_routes_to_streaming_connector() {
    // Connector A: supports QUOTE only
    let a = MockConnector::builder()
        .name("A")
        .supports_kind(AssetKind::Equity)
        .build();

    // Connector B: supports STREAM and will emit two updates for AAPL
    let b_updates = vec![
        QuoteUpdate {
            symbol: borsa_core::Symbol::new(AAPL).unwrap(),
            price: Some(usd("200.0")),
            previous_close: Some(usd("198.0")),
            ts: chrono::Utc.timestamp_opt(1, 0).unwrap(),
        },
        QuoteUpdate {
            symbol: borsa_core::Symbol::new(AAPL).unwrap(),
            price: Some(usd("201.5")),
            previous_close: Some(usd("198.0")),
            ts: chrono::Utc.timestamp_opt(2, 0).unwrap(),
        },
    ];
    let b = MockConnector::builder()
        .name("B")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(b_updates)
        .build();

    // Set registration order as [A, B], but per-symbol rule prefers B for AAPL.
    let policy = RoutingPolicyBuilder::new()
        .providers_for_symbol(AAPL, &[b.key(), a.key()])
        .build();
    let borsa = borsa::Borsa::builder()
        .with_connector(a.clone())
        .with_connector(b.clone())
        .routing_policy(policy)
        .build()
        .unwrap();

    let (_handle, mut rx) = borsa
        .stream_quotes(&[crate::helpers::instrument(AAPL, AssetKind::Equity)])
        .await
        .expect("stream started");

    let first = rx.recv().await.expect("first update");
    assert_eq!(first.symbol.as_str(), "AAPL");
    assert_eq!(first.price.unwrap().amount().to_string(), "200.0");

    let second = rx.recv().await.expect("second update");
    assert_eq!(second.ts.timestamp(), 2);
}
