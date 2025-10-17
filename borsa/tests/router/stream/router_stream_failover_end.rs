use crate::helpers::{AAPL, usd};
use borsa_core::{AssetKind, QuoteUpdate};
use chrono::TimeZone;

use crate::helpers::MockConnector;

#[tokio::test]
async fn stream_quotes_fails_over_when_first_provider_ends() {
    // First provider emits a single update then ends
    let p1_updates = vec![QuoteUpdate {
        symbol: borsa_core::Symbol::new(AAPL).unwrap(),
        price: Some(usd("100.0")),
        previous_close: None,
        ts: chrono::Utc.timestamp_opt(1, 0).unwrap(),
    }];
    let p1 = MockConnector::builder()
        .name("P1")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(p1_updates)
        .build();

    // Second provider keeps emitting more updates
    let p2_updates = vec![
        QuoteUpdate {
            symbol: borsa_core::Symbol::new(AAPL).unwrap(),
            price: Some(usd("101.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(2, 0).unwrap(),
        },
        QuoteUpdate {
            symbol: borsa_core::Symbol::new(AAPL).unwrap(),
            price: Some(usd("102.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(3, 0).unwrap(),
        },
    ];
    let p2 = MockConnector::builder()
        .name("P2")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(p2_updates)
        .build();

    let borsa = borsa::Borsa::builder()
        .with_connector(p1.clone())
        .with_connector(p2.clone())
        .prefer_for_kind(AssetKind::Equity, &[p1, p2]) // P1 has higher priority
        .build()
        .unwrap();

    let (_handle, mut rx) = borsa
        .stream_quotes(&[crate::helpers::instrument(AAPL, AssetKind::Equity)])
        .await
        .expect("stream started");

    // Expect updates from P1 then failover to P2 seamlessly
    let mut ts_list = Vec::new();
    for _ in 0..3 {
        let u = rx
            .recv()
            .await
            .expect("expected update before stream completion");
        ts_list.push(u.ts.timestamp());
    }

    assert_eq!(ts_list, vec![1, 2, 3]);
}
