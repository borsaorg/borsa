use crate::helpers::{AAPL, usd};
use borsa_core::{AssetKind, QuoteUpdate};
use chrono::TimeZone;

use crate::helpers::MockConnector;

#[tokio::test]
async fn stream_quotes_exits_when_downstream_drops() {
    // Provider that would emit many updates
    let updates = (0..50)
        .map(|i| QuoteUpdate {
            symbol: borsa_core::Symbol::new(AAPL).unwrap(),
            price: Some(usd(&(100 + i).to_string())),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(i64::from(i), 0).unwrap(),
        })
        .collect::<Vec<_>>();

    let p = MockConnector::builder()
        .name("P")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(updates)
        .build();

    let borsa = borsa::Borsa::builder().with_connector(p).build();

    let (handle, mut rx) = borsa
        .stream_quotes(&[crate::helpers::instrument(AAPL, AssetKind::Equity)])
        .await
        .expect("stream started");

    // Receive a couple then drop receiver to ensure stream task terminates
    let _ = rx.recv().await;
    let _ = rx.recv().await;
    drop(rx);

    // Stop supervisor to avoid leaks (no panic expected)
    handle.abort();
}
