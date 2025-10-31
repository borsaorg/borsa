use crate::helpers::{AAPL, instrument, usd};
use borsa_core::{AssetKind, QuoteUpdate};

use crate::helpers::MockConnector;

#[tokio::test]
async fn downstream_drop_terminates_supervisors_and_handle() {
    // Streaming connector that would emit a handful of updates
    let updates: Vec<QuoteUpdate> = (1..=5)
        .map(|t| QuoteUpdate {
            symbol: borsa_core::Symbol::new(AAPL).unwrap(),
            price: Some(usd("100.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(t, 0).unwrap(),
            volume: None,
        })
        .collect();

    let stream = MockConnector::builder()
        .name("S")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(updates)
        .build();

    let borsa = borsa::Borsa::builder()
        .with_connector(stream.clone())
        .routing_policy(borsa_core::RoutingPolicyBuilder::new().build())
        .build()
        .unwrap();

    let (handle, mut rx) = borsa
        .stream_quotes(&[instrument(AAPL, AssetKind::Equity)])
        .await
        .expect("stream started");

    // Optionally receive one update to ensure the session is active
    let _ = rx.recv().await;

    // Drop the downstream receiver; supervisors should terminate and handle should finish
    drop(rx);

    // Wait briefly for shutdown
    let start = std::time::Instant::now();
    while !handle.is_finished() {
        if start.elapsed() > std::time::Duration::from_millis(500) {
            panic!("stream handle did not finish after downstream drop");
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    // Cleanup: dropping finished handle should be a no-op
    drop(handle);
}


