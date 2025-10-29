use crate::helpers::{AAPL, MSFT, instrument, usd};
use borsa_core::{AssetKind, BorsaConnector, QuoteUpdate, RoutingPolicyBuilder};
use chrono::TimeZone;

use crate::helpers::MockConnector;

#[tokio::test]
async fn stream_quotes_assigns_symbols_per_provider() {
    // Provider X emits updates for AAPL and MSFT, but policy should assign only AAPL to X.
    let x_updates = vec![
        QuoteUpdate {
            symbol: borsa_core::Symbol::new(AAPL).unwrap(),
            price: Some(usd("10.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(1, 0).unwrap(),
            volume: None,
        },
        QuoteUpdate {
            symbol: borsa_core::Symbol::new(MSFT).unwrap(),
            price: Some(usd("11.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(2, 0).unwrap(),
            volume: None,
        },
    ];
    let x = MockConnector::builder()
        .name("X")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(x_updates)
        .build();

    // Provider Y emits MSFT; policy assigns MSFT to Y.
    let y_updates = vec![QuoteUpdate {
        symbol: borsa_core::Symbol::new(MSFT).unwrap(),
        price: Some(usd("20.0")),
        previous_close: None,
        ts: chrono::Utc.timestamp_opt(3, 0).unwrap(),
        volume: None,
    }];
    let y = MockConnector::builder()
        .name("Y")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(y_updates)
        .build();

    // Policy: AAPL -> X; MSFT -> Y
    let policy = RoutingPolicyBuilder::new()
        .providers_for_symbol(AAPL, &[x.key()])
        .providers_for_symbol(MSFT, &[y.key()])
        .build();

    let borsa = borsa::Borsa::builder()
        .with_connector(x.clone())
        .with_connector(y.clone())
        .routing_policy(policy)
        .build()
        .unwrap();

    let (_h, mut rx) = borsa
        .stream_quotes(&[
            instrument(AAPL, AssetKind::Equity),
            instrument(MSFT, AssetKind::Equity),
        ])
        .await
        .expect("stream started");

    // Expect: AAPL from X, MSFT from Y. Not MSFT from X (should be dropped by router).
    let mut got = Vec::new();
    while let Some(u) = rx.recv().await {
        got.push((
            u.symbol.as_str().to_string(),
            u.price.as_ref().map(|m| m.amount().to_string()),
        ));
        if got.len() >= 2 {
            break;
        }
    }

    assert!(got.contains(&(AAPL.to_string(), Some("10.0".to_string()))));
    assert!(got.contains(&(MSFT.to_string(), Some("20.0".to_string()))));
}
