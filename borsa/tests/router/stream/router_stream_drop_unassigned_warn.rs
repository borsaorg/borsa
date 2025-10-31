use crate::helpers::{AAPL, MSFT, instrument, usd};
use borsa_core::{AssetKind, BorsaConnector, QuoteUpdate, RoutingPolicyBuilder};
use chrono::TimeZone;

use crate::helpers::MockConnector;

#[tokio::test]
async fn stream_quotes_drops_unassigned_symbol_updates() {
    // Provider X is assigned only AAPL but will try to send MSFT as well.
    let x_updates = vec![
        QuoteUpdate {
            symbol: AAPL.clone(),
            price: Some(usd("10.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(1, 0).unwrap(),
            volume: None,
        },
        QuoteUpdate {
            symbol: MSFT.clone(),
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

    // Policy assigns AAPL to X; MSFT to no one.
    let policy = RoutingPolicyBuilder::new()
        .providers_for_symbol(&AAPL, &[x.key()])
        .build();

    let borsa = borsa::Borsa::builder()
        .with_connector(x.clone())
        .routing_policy(policy)
        .build()
        .unwrap();

    let (_h, mut rx) = borsa
        .stream_quotes(&[
            instrument(&AAPL, AssetKind::Equity),
            instrument(&MSFT, AssetKind::Equity),
        ])
        .await
        .expect("stream started");

    // Expect to receive only AAPL; MSFT should be dropped.
    let mut got = Vec::new();
    while let Some(u) = rx.recv().await {
        got.push(u.symbol.clone().to_string());
        if !got.is_empty() {
            break;
        }
    }

    assert_eq!(got, vec![AAPL.to_string()]);
}
