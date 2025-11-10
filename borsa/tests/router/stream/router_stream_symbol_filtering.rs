use crate::helpers::{AAPL, MSFT, TSLA, usd};
use borsa_core::{AssetKind, QuoteUpdate};
use chrono::TimeZone;

use crate::helpers::MockConnector;

#[tokio::test]
async fn stream_quotes_filters_to_requested_symbols_only() {
    // Provider emits mixed symbols; router should filter
    let updates = vec![
        QuoteUpdate {
            instrument: crate::helpers::instrument(&AAPL, AssetKind::Equity),
            price: Some(usd("1.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(1, 0).unwrap(),
            volume: None,
        },
        QuoteUpdate {
            instrument: crate::helpers::instrument(&TSLA, AssetKind::Equity),
            price: Some(usd("2.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(2, 0).unwrap(),
            volume: None,
        },
        QuoteUpdate {
            instrument: crate::helpers::instrument(&MSFT, AssetKind::Equity),
            price: Some(usd("3.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(3, 0).unwrap(),
            volume: None,
        },
        QuoteUpdate {
            instrument: crate::helpers::instrument(&AAPL, AssetKind::Equity),
            price: Some(usd("4.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(4, 0).unwrap(),
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
        .stream_quotes(&[crate::helpers::instrument(&AAPL, AssetKind::Equity)])
        .await
        .unwrap();

    let mut got = Vec::new();
    while let Some(u) = rx.recv().await {
        // Compare by instrument identity and timestamp
        got.push((u.instrument.clone(), u.ts.timestamp()));
        if got.len() >= 2 {
            break;
        }
    }

    assert_eq!(
        got,
        vec![
            (crate::helpers::instrument(&AAPL, AssetKind::Equity), 1),
            (crate::helpers::instrument(&AAPL, AssetKind::Equity), 4)
        ]
    );
}
