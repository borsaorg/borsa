use proptest::prelude::*;

use borsa::{BackoffConfig, Borsa};
use borsa_core::{AssetKind, Instrument, QuoteUpdate, Symbol};
use borsa_mock::{DynamicMockConnector, StreamBehavior};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
enum SymId {
    Aapl,
    Msft,
    BtcUsd,
}

impl SymId {
    fn all() -> [SymId; 3] {
        [SymId::Aapl, SymId::Msft, SymId::BtcUsd]
    }
    fn symbol(&self) -> Symbol {
        match self {
            SymId::Aapl => Symbol::new("AAPL").unwrap(),
            SymId::Msft => Symbol::new("MSFT").unwrap(),
            SymId::BtcUsd => Symbol::new("BTC-USD").unwrap(),
        }
    }
    fn kind(&self) -> AssetKind {
        match self {
            SymId::Aapl | SymId::Msft => AssetKind::Equity,
            SymId::BtcUsd => AssetKind::Crypto,
        }
    }
    fn idx(&self) -> u8 {
        match self {
            SymId::Aapl => 0,
            SymId::Msft => 1,
            SymId::BtcUsd => 2,
        }
    }
}

#[derive(Clone, Debug)]
enum Action {
    ProviderSendsUpdate { provider: u8, sym: u8, ts: i64 },
    ProviderStreamFails { provider: u8 },
    AdvanceTime { millis: u16 },
}

fn arb_action() -> impl Strategy<Value = Action> {
    use proptest::prelude::*;
    prop_oneof![
        (0u8..=2, 0u8..=2, 0i64..=10_000i64)
            .prop_map(|(p, s, ts)| Action::ProviderSendsUpdate { provider: p, sym: s, ts }),
        (0u8..=2).prop_map(|p| Action::ProviderStreamFails { provider: p }),
        (1u16..=10u16).prop_map(|ms| Action::AdvanceTime { millis: ms }),
    ]
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
            let insts: Vec<Instrument> = SymId::all().iter().map(|s| Instrument::from_symbol(&s.symbol(), s.kind()).unwrap()).collect();
            let (handle, mut rx) = borsa.stream_quotes(&insts).await.expect("stream started");

            // Model state
            let providers: [(&'static str, borsa_mock::DynamicMockController); 3] = [("P1", c1), ("P2", c2), ("P3", c3)];
            fn recompute_active(
                active: &mut HashMap<u8, Option<u8>>,
                chains: &HashMap<u8, Vec<u8>>,
                cooldown: &HashSet<u8>,
            ) {
                for (sid, chain) in chains.iter() {
                    let mut next: Option<u8> = None;
                    for &p in chain {
                        if !cooldown.contains(&p) {
                            next = Some(p);
                            break;
                        }
                    }
                    active.insert(*sid, next);
                }
            }
            // Per-symbol preference chains
            let chains: HashMap<u8, Vec<u8>> = {
                let mut m = HashMap::new();
                m.insert(SymId::Aapl.idx(), vec![0, 1, 2]); // P1,P2,P3
                m.insert(SymId::Msft.idx(), vec![1, 0, 2]); // P2,P1,P3
                m.insert(SymId::BtcUsd.idx(), vec![2, 1, 0]); // P3,P2,P1
                m
            };
            let mut cooldown: HashSet<u8> = HashSet::new();
            let mut last_ts: HashMap<u8, i64> = HashMap::new();
            // Active map is updated on init and on AdvanceTime; failure clears impacted entries
            let mut active: HashMap<u8, Option<u8>> = HashMap::new();
            for s in SymId::all().iter() { active.insert(s.idx(), Some(chains.get(&s.idx()).unwrap()[0])); }

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
                    Action::ProviderSendsUpdate { provider, sym, ts } => {
                        if sym > 2 || provider > 2 { continue; }
                        let sid = sym;
                        let sym_val = match sid { 0 => SymId::Aapl, 1 => SymId::Msft, _ => SymId::BtcUsd };
                        let expect_emit = active.get(&sid).and_then(|x| *x) == Some(provider)
                            && last_ts.get(&sid).map_or(true, |prev| ts >= *prev);
                        let ts_ch = chrono::Utc.timestamp_opt(ts, 0).unwrap();
                        let update = QuoteUpdate { symbol: sym_val.symbol(), price: None, previous_close: None, ts: ts_ch, volume: None };
                        let (name, ctrl) = &providers[provider as usize];
                        let _ = ctrl.push_update(*name, update).await;

                        let drained = drain(&mut rx).await;
                        if expect_emit {
                            assert_eq!(drained.len(), 1, "expected exactly one forwarded update for sym {} from provider {}", sid, provider);
                            assert_eq!(drained[0].symbol, sym_val.symbol());
                            assert_eq!(drained[0].ts, ts_ch);
                            last_ts.insert(sid, ts);
                        } else {
                            assert!(drained.is_empty(), "unexpected forwarded update for sym {} from provider {}", sid, provider);
                        }
                    }
                    Action::ProviderStreamFails { provider } => {
                        if provider > 2 { continue; }
                        cooldown.insert(provider);
                        // Recompute immediately – supervisor fails over right away on session end
                        recompute_active(&mut active, &chains, &cooldown);
                        let (name, ctrl) = &providers[provider as usize];
                        ctrl.fail_stream(*name).await;
                        let drained = drain(&mut rx).await;
                        assert!(drained.is_empty(), "no updates expected on failure event");
                    }
                    Action::AdvanceTime { millis } => {
                        tokio::time::advance(std::time::Duration::from_millis(millis as u64)).await;
                        // cooldown clears after the backoff tick; model that:
                        cooldown.clear();
                        // pick highest-priority provider for each symbol (failback/preemption)
                        recompute_active(&mut active, &chains, &cooldown);
                        let _ = drain(&mut rx).await;
                    }
                }
            }

            handle.stop().await;
        });
    }
}


