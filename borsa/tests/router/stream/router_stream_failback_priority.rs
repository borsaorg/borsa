use crate::helpers::{AAPL, usd};
use borsa::{BackoffConfig, Borsa};
use borsa_core::{AssetKind, BorsaConnector, QuoteUpdate, RoutingPolicyBuilder};
use chrono::TimeZone;

use crate::helpers::{MockConnector, StreamStep};

#[tokio::test]
async fn stream_quotes_fails_back_to_higher_priority_when_available() {
    // High-priority provider emits one update, ends, then later becomes available again with more updates
    let p1_steps = vec![
        StreamStep::Updates(vec![QuoteUpdate {
            symbol: borsa_core::Symbol::new(AAPL).unwrap(),
            price: Some(usd("100.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(1, 0).unwrap(),
            volume: None,
        }]),
        StreamStep::Updates(vec![
            QuoteUpdate {
                symbol: borsa_core::Symbol::new(AAPL).unwrap(),
                price: Some(usd("104.0")),
                previous_close: None,
                ts: chrono::Utc.timestamp_opt(4, 0).unwrap(),
                volume: None,
            },
            QuoteUpdate {
                symbol: borsa_core::Symbol::new(AAPL).unwrap(),
                price: Some(usd("105.0")),
                previous_close: None,
                ts: chrono::Utc.timestamp_opt(5, 0).unwrap(),
                volume: None,
            },
        ]),
    ];
    let p1 = MockConnector::builder()
        .name("P1")
        .supports_kind(AssetKind::Equity)
        .with_stream_steps(p1_steps)
        .build();

    // Low-priority provider emits a sequence; slight delay to make preemption observable
    let p2_updates = vec![
        QuoteUpdate {
            symbol: borsa_core::Symbol::new(AAPL).unwrap(),
            price: Some(usd("101.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(2, 0).unwrap(),
            volume: None,
        },
        QuoteUpdate {
            symbol: borsa_core::Symbol::new(AAPL).unwrap(),
            price: Some(usd("103.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(3, 0).unwrap(),
            volume: None,
        },
        QuoteUpdate {
            symbol: borsa_core::Symbol::new(AAPL).unwrap(),
            price: Some(usd("106.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(6, 0).unwrap(),
            volume: None,
        },
        QuoteUpdate {
            symbol: borsa_core::Symbol::new(AAPL).unwrap(),
            price: Some(usd("107.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(7, 0).unwrap(),
            volume: None,
        },
    ];
    let p2 = MockConnector::builder()
        .name("P2")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(p2_updates)
        .delay(std::time::Duration::from_millis(10))
        .build();

    let policy = RoutingPolicyBuilder::new()
        .providers_for_kind(AssetKind::Equity, &[p1.key(), p2.key()])
        .build();

    let borsa = Borsa::builder()
        .with_connector(p1.clone())
        .with_connector(p2.clone())
        .routing_policy(policy)
        .backoff(BackoffConfig {
            min_backoff_ms: 10,
            max_backoff_ms: 20,
            factor: 1,
            jitter_percent: 0,
        })
        .build()
        .unwrap();

    let (_handle, mut rx) = borsa
        .stream_quotes(&[crate::helpers::instrument(AAPL, AssetKind::Equity)])
        .await
        .expect("stream started");

    let mut ts_list = Vec::new();
    for _ in 0..5 {
        let u = rx
            .recv()
            .await
            .expect("expected update before stream completion");
        ts_list.push(u.ts.timestamp());
    }

    assert_eq!(ts_list, vec![1, 2, 3, 4, 5], "should fail back to higher-priority provider when available");
}


