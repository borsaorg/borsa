use proptest::prelude::*;

use borsa::{BackoffConfig, Borsa};
use borsa_core::{AssetKind, Instrument, QuoteUpdate, Symbol};
use borsa_mock::{DynamicMockConnector, StreamBehavior};
use chrono::TimeZone;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
enum SymId {
    Aapl,
    Msft,
    BtcUsd,
}

impl SymId {
    const fn all() -> [Self; 3] {
        [Self::Aapl, Self::Msft, Self::BtcUsd]
    }
    fn symbol(&self) -> Symbol {
        match self {
            Self::Aapl => Symbol::new("AAPL").unwrap(),
            Self::Msft => Symbol::new("MSFT").unwrap(),
            Self::BtcUsd => Symbol::new("BTC-USD").unwrap(),
        }
    }
    const fn kind(&self) -> AssetKind {
        match self {
            Self::Aapl | Self::Msft => AssetKind::Equity,
            Self::BtcUsd => AssetKind::Crypto,
        }
    }
    const fn idx(&self) -> u8 {
        match self {
            Self::Aapl => 0,
            Self::Msft => 1,
            Self::BtcUsd => 2,
        }
    }
}

#[derive(Clone, Debug)]
enum Action {
    ProviderSendsUpdate {
        provider: u8,
        sym: u8,
        ts: i64,
    },
    ProviderStreamFails {
        provider: u8,
    },
    AdvanceTime {
        millis: u16,
    },
    ProviderFailsAfterDelay {
        provider: u8,
        delay_ms: u16,
    },
    ProviderSendsBurst {
        provider: u8,
        sym: u8,
        start_ts: i64,
        count: u8,
    },
    NetworkPartition {
        providers: Vec<u8>,
        duration_ms: u16,
    },
}

fn arb_action() -> impl Strategy<Value = Action> {
    use proptest::prelude::*;
    prop_oneof![
        // Weight: 6 - Most common action is sending updates
        6 => (0u8..=2, 0u8..=2, 0i64..=10_000i64).prop_map(|(p, s, ts)| Action::ProviderSendsUpdate {
            provider: p,
            sym: s,
            ts
        }),
        // Weight: 2 - Time advances help simulate real-world timing
        2 => (1u16..=10u16).prop_map(|ms| Action::AdvanceTime { millis: ms }),
        // Weight: 1 - Immediate failures
        1 => (0u8..=2).prop_map(|p| Action::ProviderStreamFails { provider: p }),
        // Weight: 1 - Delayed failures to test race conditions
        1 => (0u8..=2, 1u16..=20u16).prop_map(|(p, delay)| Action::ProviderFailsAfterDelay {
            provider: p,
            delay_ms: delay
        }),
        // Weight: 2 - Burst of updates to test backpressure
        2 => (0u8..=2, 0u8..=2, 0i64..=10_000i64, 1u8..=5u8).prop_map(|(p, s, ts, count)| Action::ProviderSendsBurst {
            provider: p,
            sym: s,
            start_ts: ts,
            count
        }),
        // Weight: 1 - Network partitions (multiple providers failing together)
        1 => (proptest::collection::vec(0u8..=2, 1..=3), 10u16..=50u16).prop_map(|(providers, duration)| Action::NetworkPartition {
            providers,
            duration_ms: duration
        }),
    ]
}

async fn flush_rx(rx: &mut tokio::sync::mpsc::Receiver<QuoteUpdate>) -> Vec<QuoteUpdate> {
    let mut out = Vec::new();
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
    while let Ok(u) = rx.try_recv() {
        out.push(u);
    }
    out
}

fn sym_idx_from_symbol(symbol: &Symbol) -> Option<u8> {
    for sid in SymId::all() {
        if sid.symbol() == *symbol {
            return Some(sid.idx());
        }
    }
    None
}

async fn sync_assignments(
    providers: &[(&'static str, borsa_mock::DynamicMockController); 3],
    seen: &mut HashMap<u8, usize>,
    assignments: &mut HashMap<u8, HashSet<u8>>,
) {
    for (idx, (name, ctrl)) in providers.iter().enumerate() {
        let idx_u8 = u8::try_from(idx).expect("provider index fits in u8");
        let requests = ctrl.get_stream_requests(name).await;
        let start = *seen.get(&idx_u8).unwrap_or(&0);
        if start >= requests.len() {
            continue;
        }
        let entry = assignments.entry(idx_u8).or_default();
        for req in &requests[start..] {
            for inst in req {
                if let borsa_core::IdentifierScheme::Security(sec) = inst.id()
                    && let Some(sym_idx) = sym_idx_from_symbol(&sec.symbol)
                {
                    entry.insert(sym_idx);
                }
            }
        }
        seen.insert(idx_u8, requests.len());
    }
}

async fn drain_with_time(
    rx: &mut tokio::sync::mpsc::Receiver<QuoteUpdate>,
    providers: &[(&'static str, borsa_mock::DynamicMockController); 3],
    seen: &mut HashMap<u8, usize>,
    assignments: &mut HashMap<u8, HashSet<u8>>,
) -> Vec<QuoteUpdate> {
    tokio::time::advance(std::time::Duration::from_millis(1)).await;
    sync_assignments(providers, seen, assignments).await;
    flush_rx(rx).await
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 60, .. ProptestConfig::default() })]
    #[test]
    fn streaming_routing_multiplex_model(actions in proptest::collection::vec(arb_action(), 0..80)) {
        tokio_test::block_on(async move {
            tokio::time::pause();

            // Three providers in the system
            let (p1, c1) = DynamicMockConnector::new_with_controller("P1");
            let (p2, c2) = DynamicMockConnector::new_with_controller("P2");
            let (p3, c3) = DynamicMockConnector::new_with_controller("P3");
            c1.set_stream_behavior("P1", StreamBehavior::Manual).await;
            c2.set_stream_behavior("P2", StreamBehavior::Manual).await;
            c3.set_stream_behavior("P3", StreamBehavior::Manual).await;

            // Routing policy:
            // - AAPL: P1 > P2 > P3
            // - MSFT: P2 > P1 > P3
            // - BTC-USD (crypto): P3 > P2 > P1  (different group)
            let policy = borsa_core::RoutingPolicyBuilder::new()
                .providers_for_symbol(&SymId::Aapl.symbol(), &[p1.key(), p2.key(), p3.key()])
                .providers_for_symbol(&SymId::Msft.symbol(), &[p2.key(), p1.key(), p3.key()])
                .providers_for_symbol(&SymId::BtcUsd.symbol(), &[p3.key(), p2.key(), p1.key()])
                .build();

            let borsa = Borsa::builder()
                .with_connector(p1.clone())
                .with_connector(p2.clone())
                .with_connector(p3.clone())
                .routing_policy(policy)
                .backoff(BackoffConfig { min_backoff_ms: 1, max_backoff_ms: 1, factor: 1, jitter_percent: 0 })
                .build()
                .expect("borsa");

            // Start streams for all symbols across groups
            let insts: Vec<Instrument> = SymId::all().iter().map(|s| Instrument::from_symbol(s.symbol(), s.kind()).unwrap()).collect();
            let (handle, mut rx) = borsa.stream_quotes(&insts).await.expect("stream started");

            // Model state
            let providers: [(&'static str, borsa_mock::DynamicMockController); 3] = [("P1", c1), ("P2", c2), ("P3", c3)];
            let mut last_ts_by_provider: HashMap<(u8, u8), i64> = HashMap::new();
            let mut seen_requests: HashMap<u8, usize> = HashMap::new();
            let mut assignments: HashMap<u8, HashSet<u8>> = HashMap::new();
            sync_assignments(&providers, &mut seen_requests, &mut assignments).await;

            for action in actions {
                match action {
                    Action::ProviderSendsUpdate { provider, sym, ts } => {
                        if sym > 2 || provider > 2 { continue; }
                        let sid = sym;
                        let sym_val = match sid { 0 => SymId::Aapl, 1 => SymId::Msft, _ => SymId::BtcUsd };
                        let ts_ch = chrono::Utc.timestamp_opt(ts, 0).unwrap();
                        let update = QuoteUpdate { symbol: sym_val.symbol(), price: None, previous_close: None, ts: ts_ch, volume: None };
                        let (name, ctrl) = &providers[provider as usize];
                        let push_ok = ctrl.push_update(name, update).await;

                        let drained = drain_with_time(&mut rx, &providers, &mut seen_requests, &mut assignments).await;
                        let monotonic_ok = last_ts_by_provider
                            .get(&(provider, sid))
                            .is_none_or(|prev| ts >= *prev);
                        let provider_has_symbol = assignments
                            .get(&provider)
                            .is_some_and(|set| set.contains(&sid));
                        let should_route = push_ok && monotonic_ok;
                        if should_route && provider_has_symbol {
                            if drained.is_empty() {
                                if let Some(set) = assignments.get_mut(&provider) {
                                    set.remove(&sid);
                                }
                            } else {
                                assert_eq!(drained.len(), 1, "expected exactly one forwarded update for sym {sid} from provider {provider}");
                                assert_eq!(drained[0].symbol, sym_val.symbol());
                                assert_eq!(drained[0].ts, ts_ch);
                                last_ts_by_provider.insert((provider, sid), ts);
                            }
                        } else {
                            assert!(drained.is_empty(), "unexpected forwarded update for sym {sid} from provider {provider}");
                        }
                    }
                    Action::ProviderStreamFails { provider } => {
                        if provider > 2 { continue; }
                        assignments.remove(&provider);
                        // Reset per-session monotonic state for this provider
                        last_ts_by_provider.retain(|(p, _), _| *p != provider);
                        let (name, ctrl) = &providers[provider as usize];
                        ctrl.fail_stream(name).await;
                        let drained = flush_rx(&mut rx).await;
                        assert!(drained.is_empty(), "no updates expected on failure event");
                        sync_assignments(&providers, &mut seen_requests, &mut assignments).await;
                    }
                    Action::AdvanceTime { millis } => {
                        tokio::time::advance(std::time::Duration::from_millis(u64::from(millis))).await;
                        sync_assignments(&providers, &mut seen_requests, &mut assignments).await;
                        let _ = flush_rx(&mut rx).await;
                    }
                    Action::ProviderFailsAfterDelay { provider, delay_ms } => {
                        if provider > 2 { continue; }
                        tokio::time::advance(std::time::Duration::from_millis(u64::from(delay_ms))).await;
                        assignments.remove(&provider);
                        // Reset per-session monotonic state for this provider
                        last_ts_by_provider.retain(|(p, _), _| *p != provider);
                        let (name, ctrl) = &providers[provider as usize];
                        ctrl.fail_stream(name).await;
                        let drained = flush_rx(&mut rx).await;
                        assert!(drained.is_empty(), "no updates expected on delayed failure");
                        sync_assignments(&providers, &mut seen_requests, &mut assignments).await;
                    }
                    Action::ProviderSendsBurst { provider, sym, start_ts, count } => {
                        if sym > 2 || provider > 2 || count == 0 { continue; }
                        let sid = sym;
                        let sym_val = match sid { 0 => SymId::Aapl, 1 => SymId::Msft, _ => SymId::BtcUsd };
                        let (name, ctrl) = &providers[provider as usize];

                        for i in 0..count {
                            let ts = start_ts + i64::from(i);
                            let ts_ch = chrono::Utc.timestamp_opt(ts, 0).unwrap();
                            let update = QuoteUpdate { symbol: sym_val.symbol(), price: None, previous_close: None, ts: ts_ch, volume: None };
                            ctrl.push_update(name, update).await;
                        }

                        let drained = drain_with_time(&mut rx, &providers, &mut seen_requests, &mut assignments).await;

                        // Update last_ts for all received updates
                        for update in drained {
                            if let Some(update_sid) = sym_idx_from_symbol(&update.symbol) {
                                let ts = update.ts.timestamp();
                                // Since this burst originated from `provider` for `sid`, update that key only
                                if update_sid == sid {
                                    last_ts_by_provider.insert((provider, update_sid), ts);
                                }
                            }
                        }
                    }
                    Action::NetworkPartition { providers: partition_providers, duration_ms } => {
                        let valid_providers: Vec<u8> = partition_providers.into_iter().filter(|p| *p <= 2).collect();
                        if valid_providers.is_empty() { continue; }

                        for &provider in &valid_providers {
                            assignments.remove(&provider);
                            let (name, ctrl) = &providers[provider as usize];
                            ctrl.fail_stream(name).await;
                            // Reset per-session monotonic state for this provider
                            last_ts_by_provider.retain(|(p, _), _| *p != provider);
                        }

                        let drained = flush_rx(&mut rx).await;
                        assert!(drained.is_empty(), "no updates expected on partition start");

                        tokio::time::advance(std::time::Duration::from_millis(u64::from(duration_ms))).await;
                        sync_assignments(&providers, &mut seen_requests, &mut assignments).await;
                        let _ = flush_rx(&mut rx).await;
                    }
                }
            }

            handle.stop().await;
        });
    }
}
