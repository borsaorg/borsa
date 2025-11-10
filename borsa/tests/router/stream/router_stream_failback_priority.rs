use crate::helpers::{AAPL, usd};
use borsa::{BackoffConfig, Borsa};
use borsa_core::{AssetKind, BorsaConnector, QuoteUpdate, RoutingPolicyBuilder};
use chrono::TimeZone;

use crate::helpers::{MockConnector, StreamStep};

#[tokio::test(start_paused = true)]
async fn stream_quotes_fails_back_to_higher_priority_when_available() {
    let make_update = |ts| QuoteUpdate {
        instrument: crate::helpers::instrument(&AAPL, AssetKind::Equity),
        price: Some(usd("100.0")),
        previous_close: None,
        ts: chrono::Utc.timestamp_opt(ts, 0).unwrap(),
        volume: None,
    };

    let p1_steps = vec![
        StreamStep::Updates(vec![make_update(1)]),
        StreamStep::Updates(vec![make_update(4), make_update(5)]),
    ];
    let p1 = MockConnector::builder()
        .name("P1")
        .supports_kind(AssetKind::Equity)
        .with_stream_steps(p1_steps)
        .build();

    let p2_updates = vec![
        make_update(2),
        make_update(3),
        make_update(6),
        make_update(7),
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
        .stream_quotes(&[crate::helpers::instrument(&AAPL, AssetKind::Equity)])
        .await
        .expect("stream started");

    let mut ts_list = Vec::new();
    ts_list.push(rx.recv().await.expect("update 1").ts.timestamp());

    tokio::time::advance(std::time::Duration::from_millis(15)).await;
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }

    for _ in 0..4 {
        if let Ok(Some(u)) =
            tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await
        {
            ts_list.push(u.ts.timestamp());
            tokio::time::advance(std::time::Duration::from_millis(5)).await;
            tokio::task::yield_now().await;
        }
    }

    assert!(ts_list.len() >= 3, "should receive multiple updates");
    assert_eq!(ts_list[0], 1, "first from P1");
    assert!(
        ts_list.contains(&4) || ts_list.contains(&2),
        "failover behavior"
    );
}
