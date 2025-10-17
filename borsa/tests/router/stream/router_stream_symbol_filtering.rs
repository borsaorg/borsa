use crate::helpers::{AAPL, MSFT, usd};
use borsa_core::{AssetKind, QuoteUpdate};
use chrono::TimeZone;

use crate::helpers::MockConnector;

#[tokio::test]
async fn stream_quotes_filters_to_requested_symbols_only() {
    // Provider emits mixed symbols; router should filter
    let updates = vec![
        QuoteUpdate {
            symbol: borsa_core::Symbol::new(AAPL).unwrap(),
            price: Some(usd("1.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(1, 0).unwrap(),
        },
        QuoteUpdate {
            symbol: borsa_core::Symbol::new(crate::helpers::TSLA).unwrap(),
            price: Some(usd("2.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(2, 0).unwrap(),
        },
        QuoteUpdate {
            symbol: borsa_core::Symbol::new(MSFT).unwrap(),
            price: Some(usd("3.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(3, 0).unwrap(),
        },
        QuoteUpdate {
            symbol: borsa_core::Symbol::new(AAPL).unwrap(),
            price: Some(usd("4.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(4, 0).unwrap(),
        },
    ];

    let c = MockConnector::builder()
        .name("S")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(updates)
        .build();

    let borsa = borsa::Borsa::builder().with_connector(c).build();
    let (_h, mut rx) = borsa
        .stream_quotes(&[crate::helpers::instrument(AAPL, AssetKind::Equity)])
        .await
        .unwrap();

    let mut got = Vec::new();
    while let Some(u) = rx.recv().await {
        got.push((u.symbol.as_str().to_string(), u.ts.timestamp()));
        if got.len() >= 2 {
            break;
        }
    }

    assert_eq!(got, vec![(AAPL.to_string(), 1), (AAPL.to_string(), 4)]);
}
