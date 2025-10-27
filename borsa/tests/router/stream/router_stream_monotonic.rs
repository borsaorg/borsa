use crate::helpers::{AAPL, usd};
use borsa_core::{AssetKind, BorsaConnector, QuoteUpdate};
use chrono::TimeZone;

use crate::helpers::MockConnector;

#[tokio::test]
async fn stream_monotonic_drops_older_and_allows_equal() {
    let t1 = chrono::Utc.timestamp_opt(100, 0).unwrap();
    let t2_eq = t1;
    let t0_old = chrono::Utc.timestamp_opt(50, 0).unwrap();

    let updates = vec![
        QuoteUpdate { symbol: borsa_core::Symbol::new(AAPL).unwrap(), price: Some(usd("200.0")), previous_close: None, ts: t1 },
        QuoteUpdate { symbol: borsa_core::Symbol::new(AAPL).unwrap(), price: Some(usd("200.1")), previous_close: None, ts: t2_eq },
        QuoteUpdate { symbol: borsa_core::Symbol::new(AAPL).unwrap(), price: Some(usd("199.9")), previous_close: None, ts: t0_old },
    ];

    let c = MockConnector::builder()
        .name("M")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(updates)
        .build();

    let borsa = borsa::Borsa::builder()
        .with_connector(c.clone())
        .build()
        .unwrap();

    let (_handle, mut rx) = borsa
        .stream_quotes(&[crate::helpers::instrument(AAPL, AssetKind::Equity)])
        .await
        .expect("stream started");

    let first = rx.recv().await.expect("first");
    assert_eq!(first.ts, t1);
    let second = rx.recv().await.expect("second");
    assert_eq!(second.ts, t2_eq);
    // The third update (older ts) should be dropped; channel should close thereafter.
    tokio::time::timeout(std::time::Duration::from_millis(50), rx.recv())
        .await
        .ok()
        .and_then(|x| x)
        .map(|u| panic!("unexpected extra update: {u:?}"));
}

#[tokio::test]
async fn stream_monotonic_can_be_disabled() {
    let t1 = chrono::Utc.timestamp_opt(100, 0).unwrap();
    let t0_old = chrono::Utc.timestamp_opt(50, 0).unwrap();

    let updates = vec![
        QuoteUpdate { symbol: borsa_core::Symbol::new(AAPL).unwrap(), price: Some(usd("200.0")), previous_close: None, ts: t1 },
        QuoteUpdate { symbol: borsa_core::Symbol::new(AAPL).unwrap(), price: Some(usd("199.9")), previous_close: None, ts: t0_old },
    ];

    let c = MockConnector::builder()
        .name("M")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(updates)
        .build();

    let borsa = borsa::Borsa::builder()
        .with_connector(c.clone())
        .stream_enforce_monotonic_timestamps(false)
        .build()
        .unwrap();

    let (_handle, mut rx) = borsa
        .stream_quotes(&[crate::helpers::instrument(AAPL, AssetKind::Equity)])
        .await
        .expect("stream started");

    let first = rx.recv().await.expect("first");
    assert_eq!(first.ts, t1);
    let second = rx.recv().await.expect("second");
    assert_eq!(second.ts, t0_old);
}


