use crate::helpers::{AAPL, MSFT, instrument, usd};
use borsa_core::{AssetKind, BorsaConnector, QuoteUpdate, RoutingPolicyBuilder};
use chrono::TimeZone;

use crate::helpers::MockConnector;

#[tokio::test]
async fn stream_quotes_merges_and_delivers_wildcard_and_explicit_updates() {
    // Provider X is assigned only AAPL but will try to send MSFT as well.
    let x_updates = vec![
        QuoteUpdate {
            instrument: instrument(&AAPL, AssetKind::Equity),
            price: Some(usd("10.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(1, 0).unwrap(),
            volume: None,
        },
        QuoteUpdate {
            instrument: instrument(&MSFT, AssetKind::Equity),
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

    // Policy assigns AAPL explicitly to X; MSFT is eligible via wildcard.
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

    // Expect to receive both AAPL and MSFT from the merged session.
    let mut got = Vec::new();
    while let Some(u) = rx.recv().await {
        let sym = match u.instrument.id() {
            borsa_core::IdentifierScheme::Security(sec) => sec.symbol.as_str().to_string(),
            borsa_core::IdentifierScheme::Prediction(_) => "<non-security>".to_string(),
        };
        got.push(sym);
        if got.len() >= 2 {
            // Wait for both expected updates
            break;
        }
    }
    got.sort(); // Accounts for potential race condition in arrival
    assert_eq!(got, vec![AAPL.to_string(), MSFT.to_string()]);
}
