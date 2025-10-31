use crate::helpers::{instrument, usd, AAPL};
use borsa_core::{AssetKind, QuoteUpdate};
use proptest::prelude::*;

use crate::helpers::MockConnector;

proptest! {
    #![proptest_config(ProptestConfig { cases: 20, .. ProptestConfig::default() })]
    #[test]
    fn downstream_drop_finishes_handle(drop_after in 0u8..=3, num_updates in 1u8..=10) {
        tokio_test::block_on(async move {
            let updates: Vec<QuoteUpdate> = (1..=num_updates as i64)
                .map(|t| QuoteUpdate {
                    symbol: AAPL.clone(),
                    price: Some(usd("123.0")),
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

            for _ in 0..drop_after {
                let _ = rx.recv().await;
            }

            drop(rx);

            let start = std::time::Instant::now();
            while !handle.is_finished() {
                if start.elapsed() > std::time::Duration::from_millis(500) {
                    panic!("stream handle did not finish after downstream drop (drop_after={}, num_updates={})", drop_after, num_updates);
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        });
    }
}


