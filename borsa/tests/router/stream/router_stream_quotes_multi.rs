use crate::helpers::{AAPL, GOOG, MSFT, usd};
use borsa_core::{AssetKind, QuoteUpdate};
use chrono::TimeZone;

use crate::helpers::MockConnector;

#[tokio::test]
async fn stream_quotes_filters_symbols_and_emits_all() {
    // Streaming connector emits mixed symbols; router should return only requested ones
    let updates = vec![
        QuoteUpdate {
            symbol: borsa_core::Symbol::new(AAPL).unwrap(),
            price: Some(usd("120.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(10, 0).unwrap(),
        },
        QuoteUpdate {
            symbol: borsa_core::Symbol::new(MSFT).unwrap(),
            price: Some(usd("330.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(11, 0).unwrap(),
        },
        QuoteUpdate {
            symbol: borsa_core::Symbol::new(GOOG).unwrap(),
            price: Some(usd("140.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(12, 0).unwrap(),
        },
    ];

    let c = MockConnector::builder()
        .name("S")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(updates)
        .build();

    let borsa = borsa::Borsa::builder().with_connector(c).build();

    let (_h, mut rx) = borsa
        .stream_quotes(&[
            crate::helpers::instrument(AAPL, AssetKind::Equity),
            crate::helpers::instrument(MSFT, AssetKind::Equity),
        ])
        .await
        .unwrap();

    let mut got = Vec::new();
    while let Some(u) = rx.recv().await {
        got.push(u.symbol.as_str().to_string());
        if got.len() >= 2 {
            break;
        }
    }
    got.sort();
    assert_eq!(got, vec!["AAPL", "MSFT"]);
}
