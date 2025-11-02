use crate::helpers::{AAPL, instrument, usd};
use borsa::{BackoffConfig, Borsa};
use borsa_core::{AssetKind, BorsaConnector, QuoteUpdate, RoutingPolicyBuilder};
use chrono::TimeZone;

use crate::helpers::{MockConnector, StreamStep};

#[tokio::test(start_paused = true)]
async fn rapid_provider_flapping_no_resource_leak() {
    // Provider that fails and recovers rapidly multiple times
    let flapping_steps = vec![
        // First connection: emit one update, then end
        StreamStep::Updates(vec![QuoteUpdate {
            symbol: AAPL.clone(),
            price: Some(usd("100.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(1, 0).unwrap(),
            volume: None,
        }]),
        // Second connection: emit one update, then end
        StreamStep::Updates(vec![QuoteUpdate {
            symbol: AAPL.clone(),
            price: Some(usd("101.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(2, 0).unwrap(),
            volume: None,
        }]),
        // Third connection: emit one update, then end
        StreamStep::Updates(vec![QuoteUpdate {
            symbol: AAPL.clone(),
            price: Some(usd("102.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(3, 0).unwrap(),
            volume: None,
        }]),
    ];

    let provider = MockConnector::builder()
        .name("Flapping")
        .supports_kind(AssetKind::Equity)
        .with_stream_steps(flapping_steps)
        .build();

    let policy = RoutingPolicyBuilder::new()
        .providers_for_kind(AssetKind::Equity, &[provider.key()])
        .build();

    let borsa = Borsa::builder()
        .with_connector(provider.clone())
        .routing_policy(policy)
        .backoff(BackoffConfig {
            min_backoff_ms: 10,
            max_backoff_ms: 20,
            factor: 1,
            jitter_percent: 0,
        })
        .build()
        .unwrap();

    let (handle, mut rx) = borsa
        .stream_quotes(&[instrument(&AAPL, AssetKind::Equity)])
        .await
        .expect("stream started");

    // Collect first update
    let first = rx.recv().await.expect("first update");
    assert_eq!(first.ts.timestamp(), 1);

    // Advance time to trigger backoff and reconnection
    tokio::time::advance(std::time::Duration::from_millis(15)).await;
    tokio::task::yield_now().await;

    let second = rx.recv().await.expect("second update after reconnect");
    assert_eq!(second.ts.timestamp(), 2);

    // Advance time again for third reconnection
    tokio::time::advance(std::time::Duration::from_millis(15)).await;
    tokio::task::yield_now().await;

    let third = rx.recv().await.expect("third update after reconnect");
    assert_eq!(third.ts.timestamp(), 3);

    // Verify handle completes cleanly (no leaked tasks)
    handle.stop().await;
}

#[tokio::test(start_paused = true)]
async fn rapid_flapping_with_multiple_providers() {
    // Two providers that flap at different rates
    let p1_steps = vec![
        StreamStep::Updates(vec![QuoteUpdate {
            symbol: AAPL.clone(),
            price: Some(usd("100.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(1, 0).unwrap(),
            volume: None,
        }]),
        StreamStep::Updates(vec![QuoteUpdate {
            symbol: AAPL.clone(),
            price: Some(usd("104.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(4, 0).unwrap(),
            volume: None,
        }]),
    ];

    let p2_steps = vec![StreamStep::Updates(vec![QuoteUpdate {
        symbol: AAPL.clone(),
        price: Some(usd("101.0")),
        previous_close: None,
        ts: chrono::Utc.timestamp_opt(2, 0).unwrap(),
        volume: None,
    }])];

    let p1 = MockConnector::builder()
        .name("P1")
        .supports_kind(AssetKind::Equity)
        .with_stream_steps(p1_steps)
        .build();

    let p2 = MockConnector::builder()
        .name("P2")
        .supports_kind(AssetKind::Equity)
        .with_stream_steps(p2_steps)
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

    let (handle, mut rx) = borsa
        .stream_quotes(&[instrument(&AAPL, AssetKind::Equity)])
        .await
        .expect("stream started");

    // P1 sends first update (ts=1)
    let first = rx.recv().await.expect("first update");
    assert_eq!(first.ts.timestamp(), 1);

    // P1 ends, P2 takes over (ts=2)
    tokio::time::advance(std::time::Duration::from_millis(15)).await;
    tokio::task::yield_now().await;

    let second = rx.recv().await.expect("second update");
    assert_eq!(second.ts.timestamp(), 2);

    // P2 ends, P1 reconnects (ts=4)
    tokio::time::advance(std::time::Duration::from_millis(15)).await;
    tokio::task::yield_now().await;

    let third = rx.recv().await.expect("third update after P1 reconnect");
    assert_eq!(third.ts.timestamp(), 4);

    handle.stop().await;
}
