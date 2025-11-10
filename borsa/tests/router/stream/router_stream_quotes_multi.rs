use crate::helpers::{AAPL, GOOG, MSFT, usd};
use borsa_core::{AssetKind, QuoteUpdate};
use chrono::TimeZone;

use crate::helpers::MockConnector;

#[tokio::test]
async fn stream_quotes_filters_symbols_and_emits_all() {
    // Streaming connector emits mixed symbols; router should return only requested ones
    let updates = vec![
        QuoteUpdate {
            instrument: crate::helpers::instrument(&AAPL, AssetKind::Equity),
            price: Some(usd("120.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(10, 0).unwrap(),
            volume: None,
        },
        QuoteUpdate {
            instrument: crate::helpers::instrument(&MSFT, AssetKind::Equity),
            price: Some(usd("330.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(11, 0).unwrap(),
            volume: None,
        },
        QuoteUpdate {
            instrument: crate::helpers::instrument(&GOOG, AssetKind::Equity),
            price: Some(usd("140.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(12, 0).unwrap(),
            volume: None,
        },
    ];

    let c = MockConnector::builder()
        .name("S")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(updates)
        .build();

    let borsa = borsa::Borsa::builder().with_connector(c).build().unwrap();

    let (_h, mut rx) = borsa
        .stream_quotes(&[
            crate::helpers::instrument(&AAPL, AssetKind::Equity),
            crate::helpers::instrument(&MSFT, AssetKind::Equity),
        ])
        .await
        .unwrap();

    let mut got = Vec::new();
    while let Some(u) = rx.recv().await {
        let sym_str = match u.instrument.id() {
            borsa_core::IdentifierScheme::Security(sec) => sec.symbol.as_str().to_string(),
            borsa_core::IdentifierScheme::Prediction(_) => "<non-security>".to_string(),
        };
        got.push(sym_str);
        if got.len() >= 2 {
            break;
        }
    }
    got.sort();
    assert_eq!(got, vec!["AAPL", "MSFT"]);
}
