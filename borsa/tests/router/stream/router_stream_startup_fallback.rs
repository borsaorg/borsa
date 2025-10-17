use crate::helpers::{AAPL, ts, usd};
use borsa_core::{AssetKind, QuoteUpdate};
use chrono::TimeZone;

use crate::helpers::MockConnector;

#[tokio::test]
async fn stream_quotes_falls_back_when_first_cannot_start() {
    // First provider claims STREAM but fails to start
    let failing = MockConnector::builder()
        .name("F")
        .supports_kind(AssetKind::Equity)
        .will_fail_stream_start("intentional-startup-failure")
        .build();

    // Second provider starts successfully and emits data
    let ok_updates = vec![
        QuoteUpdate {
            symbol: borsa_core::Symbol::new(AAPL).unwrap(),
            price: Some(usd("150.0")),
            previous_close: None,
            ts: chrono::Utc
                .timestamp_opt(ts(1970, 1, 1, 0, 0, 10), 0)
                .unwrap(),
        },
        QuoteUpdate {
            symbol: borsa_core::Symbol::new(AAPL).unwrap(),
            price: Some(usd("151.0")),
            previous_close: None,
            ts: chrono::Utc
                .timestamp_opt(ts(1970, 1, 1, 0, 0, 11), 0)
                .unwrap(),
        },
    ];
    let ok = MockConnector::builder()
        .name("S")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(ok_updates)
        .build();

    let borsa = borsa::Borsa::builder()
        .with_connector(failing.clone())
        .with_connector(ok.clone())
        .prefer_for_kind(AssetKind::Equity, &[failing, ok]) // Prefer failing first
        .build()
        .unwrap();

    let (_h, mut rx) = borsa
        .stream_quotes(&[crate::helpers::instrument(AAPL, AssetKind::Equity)])
        .await
        .expect("stream started");

    let first = rx.recv().await.expect("first update");
    assert_eq!(first.ts.timestamp(), 10);
}
