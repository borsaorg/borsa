use crate::helpers::{AAPL, instrument, usd};
use borsa::Borsa;
use borsa_core::{AssetKind, QuoteUpdate};
use chrono::TimeZone;

use crate::helpers::MockConnector;

#[tokio::test]
async fn slow_consumer_handles_backpressure() {
    // Provider sends many updates rapidly
    let updates: Vec<QuoteUpdate> = (1..=100)
        .map(|t| QuoteUpdate {
            instrument: instrument(&AAPL, AssetKind::Equity),
            price: Some(usd("100.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(t, 0).unwrap(),
            volume: None,
        })
        .collect();

    let provider = MockConnector::builder()
        .name("FastProvider")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(updates)
        .delay(std::time::Duration::from_micros(100)) // Very fast updates
        .build();

    let borsa = Borsa::builder()
        .with_connector(provider.clone())
        .build()
        .unwrap();

    let (handle, mut rx) = borsa
        .stream_quotes(&[instrument(&AAPL, AssetKind::Equity)])
        .await
        .expect("stream started");

    // Slow consumer: only read a few updates with delays
    let mut received_count = 0;
    for _ in 0..5 {
        if rx.recv().await.is_some() {
            received_count += 1;
            // Simulate slow processing
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    }

    assert_eq!(received_count, 5);

    // Stop the stream - consumes handle
    handle.stop().await;
}

#[tokio::test]
async fn consumer_not_reading_still_allows_graceful_shutdown() {
    let updates: Vec<QuoteUpdate> = (1..=50)
        .map(|t| QuoteUpdate {
            instrument: instrument(&AAPL, AssetKind::Equity),
            price: Some(usd("100.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(t, 0).unwrap(),
            volume: None,
        })
        .collect();

    let provider = MockConnector::builder()
        .name("Provider")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(updates)
        .delay(std::time::Duration::from_millis(1))
        .build();

    let borsa = Borsa::builder()
        .with_connector(provider.clone())
        .build()
        .unwrap();

    let (handle, rx) = borsa
        .stream_quotes(&[instrument(&AAPL, AssetKind::Equity)])
        .await
        .expect("stream started");

    // Consumer doesn't read at all - just holds the receiver
    // Let some time pass for updates to accumulate
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Drop receiver without reading
    drop(rx);

    // Verify handle finishes cleanly
    let start = std::time::Instant::now();
    while !handle.is_finished() {
        assert!(
            start.elapsed() <= std::time::Duration::from_millis(500),
            "handle did not finish after dropping non-reading consumer"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn channel_saturation_doesnt_block_supervisor() {
    // Test that if channel fills up, supervisor logic still works
    let updates: Vec<QuoteUpdate> = (1..=1000)
        .map(|t| QuoteUpdate {
            instrument: instrument(&AAPL, AssetKind::Equity),
            price: Some(usd("100.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(t, 0).unwrap(),
            volume: None,
        })
        .collect();

    let provider = MockConnector::builder()
        .name("HighVolume")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(updates)
        .delay(std::time::Duration::from_micros(10)) // Very rapid
        .build();

    let borsa = Borsa::builder()
        .with_connector(provider.clone())
        .build()
        .unwrap();

    let (handle, mut rx) = borsa
        .stream_quotes(&[instrument(&AAPL, AssetKind::Equity)])
        .await
        .expect("stream started");

    // Read slowly while provider sends fast
    let mut received = 0;
    for _ in 0..10 {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        if rx.recv().await.is_some() {
            received += 1;
        }
    }

    // Should have received some updates
    assert!(received > 0);

    // Stop should work even with potential backpressure
    handle.stop().await;
}

#[tokio::test]
async fn intermittent_slow_consumer() {
    // Consumer alternates between fast and slow reading
    let updates: Vec<QuoteUpdate> = (1..=50)
        .map(|t| QuoteUpdate {
            instrument: instrument(&AAPL, AssetKind::Equity),
            price: Some(usd(&format!("{}.0", 100 + t))),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(t, 0).unwrap(),
            volume: None,
        })
        .collect();

    let provider = MockConnector::builder()
        .name("Provider")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(updates)
        .delay(std::time::Duration::from_millis(2))
        .build();

    let borsa = Borsa::builder()
        .with_connector(provider.clone())
        .build()
        .unwrap();

    let (handle, mut rx) = borsa
        .stream_quotes(&[instrument(&AAPL, AssetKind::Equity)])
        .await
        .expect("stream started");

    let mut received = Vec::new();

    // Read fast for first 5
    for _ in 0..5 {
        if let Some(update) = rx.recv().await {
            received.push(update.ts.timestamp());
        }
    }

    // Read slowly for next 5
    for _ in 0..5 {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        if let Some(update) = rx.recv().await {
            received.push(update.ts.timestamp());
        }
    }

    // Read fast again
    for _ in 0..5 {
        if let Some(update) = rx.recv().await {
            received.push(update.ts.timestamp());
        }
    }

    // Should have received updates throughout
    assert_eq!(received.len(), 15);

    handle.stop().await;
}
