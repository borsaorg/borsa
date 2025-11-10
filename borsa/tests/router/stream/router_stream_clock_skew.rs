use crate::helpers::{AAPL, instrument, usd};
use borsa::Borsa;
use borsa_core::{AssetKind, QuoteUpdate};
use chrono::TimeZone;

use crate::helpers::MockConnector;

#[tokio::test]
async fn clock_skew_monotonic_filter_drops_older_timestamps() {
    // Single provider sends updates with clock skew (out of order timestamps)
    let updates = vec![
        QuoteUpdate {
            instrument: instrument(&AAPL, AssetKind::Equity),
            price: Some(usd("100.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(1100, 0).unwrap(), // Future timestamp
            volume: None,
        },
        QuoteUpdate {
            instrument: instrument(&AAPL, AssetKind::Equity),
            price: Some(usd("101.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(1000, 0).unwrap(), // Older timestamp
            volume: None,
        },
        QuoteUpdate {
            instrument: instrument(&AAPL, AssetKind::Equity),
            price: Some(usd("102.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(1200, 0).unwrap(), // Newer timestamp
            volume: None,
        },
    ];

    let provider = MockConnector::builder()
        .name("ClockSkewed")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(updates)
        .build();

    let borsa = Borsa::builder()
        .with_connector(provider.clone())
        .build()
        .unwrap();

    let (_handle, mut rx) = borsa
        .stream_quotes(&[instrument(&AAPL, AssetKind::Equity)])
        .await
        .expect("stream started");

    // With monotonic filtering enabled (default)
    let first = rx.recv().await.expect("first update");
    assert_eq!(first.ts.timestamp(), 1100);

    // Second update (ts=1000) should be dropped as older
    // Third update (ts=1200) should pass through
    let second = rx.recv().await.expect("second update");
    assert_eq!(second.ts.timestamp(), 1200); // Skipped ts=1000, got ts=1200

    // No more updates
    let timeout_result =
        tokio::time::timeout(std::time::Duration::from_millis(50), rx.recv()).await;
    assert!(timeout_result.is_err(), "no more updates expected");
}

#[tokio::test]
async fn clock_skew_without_monotonic_filter() {
    // Single provider with out-of-order timestamps, monotonic filter disabled
    let updates = vec![
        QuoteUpdate {
            instrument: instrument(&AAPL, AssetKind::Equity),
            price: Some(usd("100.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(1100, 0).unwrap(),
            volume: None,
        },
        QuoteUpdate {
            instrument: instrument(&AAPL, AssetKind::Equity),
            price: Some(usd("101.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(1000, 0).unwrap(), // Older - should NOT be dropped
            volume: None,
        },
    ];

    let provider = MockConnector::builder()
        .name("Provider")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(updates)
        .build();

    let borsa = Borsa::builder()
        .with_connector(provider.clone())
        .stream_enforce_monotonic_timestamps(false) // Disable monotonic filter
        .build()
        .unwrap();

    let (_handle, mut rx) = borsa
        .stream_quotes(&[instrument(&AAPL, AssetKind::Equity)])
        .await
        .expect("stream started");

    // Without monotonic filter, both updates arrive in order sent
    let first = rx.recv().await.expect("first update");
    assert_eq!(first.ts.timestamp(), 1100);

    let second = rx.recv().await.expect("second update");
    assert_eq!(second.ts.timestamp(), 1000); // Older timestamp allowed
}

#[tokio::test]
async fn equal_timestamps_allowed_with_monotonic_filter() {
    // Provider sends multiple updates with the same timestamp
    let common_ts = chrono::Utc.timestamp_opt(1000, 0).unwrap();

    let updates = vec![
        QuoteUpdate {
            instrument: instrument(&AAPL, AssetKind::Equity),
            price: Some(usd("100.0")),
            previous_close: None,
            ts: common_ts,
            volume: None,
        },
        QuoteUpdate {
            instrument: instrument(&AAPL, AssetKind::Equity),
            price: Some(usd("100.5")),
            previous_close: None,
            ts: common_ts, // Same timestamp
            volume: None,
        },
        QuoteUpdate {
            instrument: instrument(&AAPL, AssetKind::Equity),
            price: Some(usd("101.0")),
            previous_close: None,
            ts: common_ts, // Same timestamp again
            volume: None,
        },
    ];

    let provider = MockConnector::builder()
        .name("Provider")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(updates)
        .build();

    let borsa = Borsa::builder()
        .with_connector(provider.clone())
        .build()
        .unwrap();

    let (_handle, mut rx) = borsa
        .stream_quotes(&[instrument(&AAPL, AssetKind::Equity)])
        .await
        .expect("stream started");

    // All updates with equal timestamps should pass through
    for _ in 0..3 {
        let update = rx.recv().await.expect("update with equal timestamp");
        assert_eq!(update.ts, common_ts);
    }
}
