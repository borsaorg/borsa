use crate::helpers::{instrument, usd, AAPL};
use borsa::{BackoffConfig, Borsa};
use borsa_core::{AssetKind, QuoteUpdate, RoutingPolicyBuilder};

use crate::helpers::{MockConnector, StreamStep};

#[tokio::test]
async fn cooldown_skips_provider_until_after_backoff_tick() {
    // High-priority provider emits one update, then ends; later becomes available again
    let p1_steps = vec![
        StreamStep::Updates(vec![QuoteUpdate {
            symbol: borsa_core::Symbol::new(AAPL).unwrap(),
            price: Some(usd("100.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(1, 0).unwrap(),
            volume: None,
        }]),
        // After cooldown/backoff we will return again
        StreamStep::Updates(vec![QuoteUpdate {
            symbol: borsa_core::Symbol::new(AAPL).unwrap(),
            price: Some(usd("150.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(150, 0).unwrap(),
            volume: None,
        }]),
    ];
    let p1 = MockConnector::builder()
        .name("P1")
        .supports_kind(AssetKind::Equity)
        .with_stream_steps(p1_steps)
        .build();

    // Lower-priority provider provides a few fast updates
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
            price: Some(usd("102.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(3, 0).unwrap(),
            volume: None,
        },
        QuoteUpdate {
            symbol: borsa_core::Symbol::new(AAPL).unwrap(),
            price: Some(usd("103.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(4, 0).unwrap(),
            volume: None,
        },
    ];
    let p2 = MockConnector::builder()
        .name("P2")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(p2_updates)
        .delay(std::time::Duration::from_millis(1))
        .build();

    // P1 higher priority than P2
    let policy = RoutingPolicyBuilder::new()
        .providers_for_kind(AssetKind::Equity, &[p1.key(), p2.key()])
        .build();

    let borsa = Borsa::builder()
        .with_connector(p1.clone())
        .with_connector(p2.clone())
        .routing_policy(policy)
        .backoff(BackoffConfig {
            min_backoff_ms: 40,
            max_backoff_ms: 80,
            factor: 1,
            jitter_percent: 0,
        })
        .build()
        .unwrap();

    let (_h, mut rx) = borsa
        .stream_quotes(&[instrument(AAPL, AssetKind::Equity)])
        .await
        .expect("stream started");

    let mut ts = Vec::new();
    for _ in 0..4 {
        let u = rx.recv().await.expect("update");
        ts.push(u.ts.timestamp());
    }

    // Expect that after P1 ends, we see P2's next two updates before P1 is retried (due to cooldown/backoff)
    assert_eq!(ts, vec![1, 2, 3, 4]);
}

#[tokio::test]
async fn cooldown_handles_consecutive_provider_failures_without_immediate_retry() {
    use chrono::TimeZone;

    // P0: connect, emit 1 update, end; later will return again after cooldown
    let p0_steps = vec![
        StreamStep::Updates(vec![QuoteUpdate {
            symbol: borsa_core::Symbol::new(AAPL).unwrap(),
            price: Some(usd("100.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(1, 0).unwrap(),
            volume: None,
        }]),
        // Next attempt should occur only after backoff tick
        StreamStep::Updates(vec![QuoteUpdate {
            symbol: borsa_core::Symbol::new(AAPL).unwrap(),
            price: Some(usd("200.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(200, 0).unwrap(),
            volume: None,
        }]),
    ];
    let p0 = MockConnector::builder()
        .name("P0")
        .supports_kind(AssetKind::Equity)
        .with_stream_steps(p0_steps)
        .build();

    // P1: connect, emit 1 update, end
    let p1_steps = vec![StreamStep::Updates(vec![QuoteUpdate {
        symbol: borsa_core::Symbol::new(AAPL).unwrap(),
        price: Some(usd("110.0")),
        previous_close: None,
        ts: chrono::Utc.timestamp_opt(2, 0).unwrap(),
        volume: None,
    }])];
    let p1 = MockConnector::builder()
        .name("P1")
        .supports_kind(AssetKind::Equity)
        .with_stream_steps(p1_steps)
        .build();

    // P2: always fail to start to force the supervisor to consider retrying P0/P1
    let p2 = MockConnector::builder()
        .name("P2")
        .supports_kind(AssetKind::Equity)
        .will_fail_stream_start("fail-start")
        .build();

    // Priority: P0 > P1 > P2
    let policy = RoutingPolicyBuilder::new()
        .providers_for_kind(AssetKind::Equity, &[p0.key(), p1.key(), p2.key()])
        .build();

    let borsa = Borsa::builder()
        .with_connector(p0.clone())
        .with_connector(p1.clone())
        .with_connector(p2.clone())
        .routing_policy(policy)
        .backoff(BackoffConfig {
            min_backoff_ms: 40,
            max_backoff_ms: 80,
            factor: 1,
            jitter_percent: 0,
        })
        .build()
        .unwrap();

    let (_h, mut rx) = borsa
        .stream_quotes(&[instrument(AAPL, AssetKind::Equity)])
        .await
        .expect("stream started");

    // First two updates should be from P0 then P1
    let first = rx.recv().await.expect("update").ts.timestamp();
    let second = rx.recv().await.expect("update").ts.timestamp();
    assert_eq!((first, second), (1, 2));

    // Ensure the third update is NOT delivered immediately (i.e., before backoff tick)
    let early = tokio::time::timeout(std::time::Duration::from_millis(10), rx.recv()).await;
    assert!(early.is_err(), "received an update before cooldown backoff tick");

    // Next update should come after backoff tick from P0 (ts=200)
    let third = tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv())
        .await
        .expect("timely third update")
        .expect("third update present")
        .ts
        .timestamp();
    assert_eq!(third, 200);
}


