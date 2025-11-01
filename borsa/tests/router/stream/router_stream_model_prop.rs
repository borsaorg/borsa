use proptest::prelude::*;

use borsa::{BackoffConfig, Borsa};
use borsa_core::{AssetKind, Instrument, QuoteUpdate, Symbol};
use borsa_mock::{DynamicMockConnector, StreamBehavior};

fn arb_action() -> impl Strategy<Value = Action> {
    use proptest::prelude::*;
    prop_oneof![
        // Provider indices 0..=2 map to P1, P2, P3; timestamps in a modest range
        (0u8..=2, 0i64..=1000i64).prop_map(|(p, ts)| Action::ProviderSendsUpdate { provider: p, ts }),
        (0u8..=2).prop_map(|p| Action::ProviderStreamFails { provider: p }),
        (1u16..=10u16).prop_map(|ms| Action::AdvanceTime { millis: ms }),
    ]
}

#[derive(Clone, Debug)]
enum Action {
    ProviderSendsUpdate { provider: u8, ts: i64 },
    ProviderStreamFails { provider: u8 },
    AdvanceTime { millis: u16 },
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 30, .. ProptestConfig::default() })]
    #[test]
    fn streaming_model_holds(actions in proptest::collection::vec(arb_action(), 0..60)) {
        tokio_test::block_on(async move {
            // Control time deterministically for backoff/failback
            tokio::time::pause();

            // Three providers with strict priority P1 > P2 > P3
            let (p1, c1) = DynamicMockConnector::new_with_controller("P1");
            let (p2, c2) = DynamicMockConnector::new_with_controller("P2");
            let (p3, c3) = DynamicMockConnector::new_with_controller("P3");

            c1.set_stream_behavior("P1", StreamBehavior::Manual).await;
            c2.set_stream_behavior("P2", StreamBehavior::Manual).await;
            c3.set_stream_behavior("P3", StreamBehavior::Manual).await;

            let policy = borsa_core::RoutingPolicyBuilder::new()
                .providers_for_kind(AssetKind::Equity, &[p1.key(), p2.key(), p3.key()])
                .build();

            let borsa = Borsa::builder()
                .with_connector(p1.clone())
                .with_connector(p2.clone())
                .with_connector(p3.clone())
                .routing_policy(policy)
                .backoff(BackoffConfig {
                    min_backoff_ms: 1,
                    max_backoff_ms: 1,
                    factor: 1,
                    jitter_percent: 0,
                })
                .build()
                .expect("borsa");

            let sym = Symbol::new("AAPL").unwrap();
            let inst = Instrument::from_symbol(&sym, AssetKind::Equity).expect("inst");

            let (handle, mut rx) = borsa
                .stream_quotes(&[inst])
                .await
                .expect("stream started");

            // Model state
            let mut cooldown: std::collections::HashSet<u8> = std::collections::HashSet::new();
            let providers: [(&'static str, borsa_mock::DynamicMockController); 3] = [("P1", c1), ("P2", c2), ("P3", c3)];
            let mut last_ts: Option<i64> = None;

            let active_idx = |cooldown: &std::collections::HashSet<u8>| -> Option<u8> {
                for i in 0..3u8 {
                    if !cooldown.contains(&i) { return Some(i); }
                }
                None
            };

            // Helper to drain the SUT receiver without blocking
            async fn drain(mut rx: &mut tokio::sync::mpsc::Receiver<QuoteUpdate>) -> Vec<QuoteUpdate> {
                let mut out = Vec::new();
                for _ in 0..5 { tokio::task::yield_now().await; }
                loop {
                    match rx.try_recv() {
                        Ok(u) => out.push(u),
                        Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                        Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
                    }
                }
                out
            }

            for action in actions {
                match action {
                    Action::ProviderSendsUpdate { provider, ts } => {
                        let expect_emit = active_idx(&cooldown) == Some(provider)
                            && last_ts.map_or(true, |prev| ts >= prev);

                        let ts_ch = chrono::Utc.timestamp_opt(ts, 0).unwrap();
                        let update = QuoteUpdate {
                            symbol: sym.clone(),
                            price: None,
                            previous_close: None,
                            ts: ts_ch,
                            volume: None,
                        };

                        let (name, ctrl) = &providers[provider as usize];
                        let _ = ctrl.push_update(*name, update).await;

                        let drained = drain(&mut rx).await;
                        if expect_emit {
                            assert_eq!(drained.len(), 1, "expected exactly one forwarded update");
                            assert_eq!(drained[0].ts, ts_ch, "timestamp must match");
                            last_ts = Some(ts);
                        } else {
                            assert!(drained.is_empty(), "unexpected forwarded update from provider {} at ts {}", provider, ts);
                        }
                    }
                    Action::ProviderStreamFails { provider } => {
                        cooldown.insert(provider);
                        let (name, ctrl) = &providers[provider as usize];
                        ctrl.fail_stream(*name).await;
                        // No output is expected immediately; ensure no stray messages
                        let drained = drain(&mut rx).await;
                        assert!(drained.is_empty(), "unexpected updates during provider failure");
                    }
                    Action::AdvanceTime { millis } => {
                        tokio::time::advance(std::time::Duration::from_millis(millis as u64)).await;
                        // Supervisor clears cooldowns on tick
                        cooldown.clear();
                        let _ = drain(&mut rx).await; // nothing expected
                    }
                }
            }

            handle.stop().await;
        });
    }
}


