use crate::helpers::{AAPL, ts, usd};
use borsa::BackoffConfig;
use borsa_core::{AssetKind, BorsaConnector, QuoteUpdate, RoutingPolicyBuilder};
use chrono::TimeZone;

use crate::helpers::MockConnector;

#[tokio::test]
async fn stream_quotes_falls_back_when_first_cannot_start() {
    // First provider claims STREAM but fails to start
    let failing = MockConnector::builder()
        .name("F")
        .supports_kind(AssetKind::Equity)
        .will_fail_stream_start("intentional-startup-failure")
        .build();

    // Second provider starts successfully and emits data
    let ok_updates = vec![
        QuoteUpdate {
            symbol: AAPL.clone(),
            price: Some(usd("150.0")),
            previous_close: None,
            ts: chrono::Utc
                .timestamp_opt(ts(1970, 1, 1, 0, 0, 10), 0)
                .unwrap(),
            volume: None,
        },
        QuoteUpdate {
            symbol: AAPL.clone(),
            price: Some(usd("151.0")),
            previous_close: None,
            ts: chrono::Utc
                .timestamp_opt(ts(1970, 1, 1, 0, 0, 11), 0)
                .unwrap(),
            volume: None,
        },
    ];
    let ok = MockConnector::builder()
        .name("S")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(ok_updates)
        .build();

    let policy = RoutingPolicyBuilder::new()
        .providers_for_kind(AssetKind::Equity, &[failing.key(), ok.key()])
        .build();
    let borsa = borsa::Borsa::builder()
        .with_connector(failing.clone())
        .with_connector(ok.clone())
        .routing_policy(policy)
        .provider_timeout(std::time::Duration::from_millis(50))
        .backoff(BackoffConfig {
            min_backoff_ms: 1,
            max_backoff_ms: 10,
            factor: 1,
            jitter_percent: 0,
        })
        .build()
        .unwrap();

    let (handle, mut rx) = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        borsa.stream_quotes(&[crate::helpers::instrument(&AAPL, AssetKind::Equity)]),
    )
    .await
    .expect("stream setup should not hang")
    .expect("stream started");

    let first = rx.recv().await.expect("first update");
    assert_eq!(first.ts.timestamp(), 10);

    handle.stop().await;
}
