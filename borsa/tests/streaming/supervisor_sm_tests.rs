use std::sync::Arc;

use borsa::router::streaming::supervisor_sm::{Action, Event, Phase, ProviderState, Supervisor};

#[test]
fn backoff_tick_clears_cooldown_and_schedules_tick() {
    let providers = vec![
        ProviderState::InCooldown { failed_at: std::time::Instant::now() },
        ProviderState::Idle,
    ];
    let sup = Supervisor {
        providers,
        provider_instruments: vec![Vec::new(), Vec::new()],
        provider_allow: vec![std::collections::HashSet::new(), std::collections::HashSet::new()],
        required_symbols: std::collections::HashSet::new(),
        start_index: 0,
        backoff_ms: 100,
        min_backoff_ms: 100,
        max_backoff_ms: 10_000,
        factor: 2,
        jitter_percent: 0,
        attempted_since_last_tick: false,
        phase: Phase::Running,
    };

    let (next, actions) = sup.handle(Event::BackoffTick);

    // Provider 0 should be cleared to IdleFromCooldown
    assert!(matches!(next.providers[0], ProviderState::IdleFromCooldown));
    // Backoff remains same because no attempts since last tick
    assert_eq!(next.backoff_ms, 100);
    // One ScheduleBackoffTick action
    assert!(actions.iter().any(|a| matches!(a, Action::ScheduleBackoffTick { .. })));
}

#[test]
fn startup_failure_accumulates_error_and_does_not_notify_without_tick() {
    let providers = vec![ProviderState::Idle];
    let sup = Supervisor {
        providers,
        provider_instruments: vec![Vec::new()],
        provider_allow: vec![std::collections::HashSet::new()],
        required_symbols: std::collections::HashSet::new(),
        start_index: 0,
        backoff_ms: 100,
        min_backoff_ms: 100,
        max_backoff_ms: 10_000,
        factor: 2,
        jitter_percent: 0,
        attempted_since_last_tick: false,
        phase: Phase::Startup { initial_tx: None, accumulated_errors: Vec::new() },
    };

    // Create a synthetic error by reusing unsupported capability constructor if available via collapse later; here we only assert action absence
    let err = borsa::core::BorsaError::unsupported("stream".to_string());
    let (next, actions) = sup.handle(Event::ProviderStartFailed { id: 0, error: err });

    // Remain in Startup, no actions produced
    assert!(matches!(next.phase, Phase::Startup { .. }));
    assert!(actions.is_empty());
}

#[test]
fn startup_tick_without_actives_notifies_error_and_terminates() {
    let providers = vec![ProviderState::Idle];
    let (tx, _rx) = tokio::sync::oneshot::channel::<Result<(), borsa::core::BorsaError>>();
    let sup = Supervisor {
        providers,
        provider_instruments: vec![Vec::new()],
        provider_allow: vec![std::collections::HashSet::new()],
        required_symbols: std::collections::HashSet::new(),
        start_index: 0,
        backoff_ms: 100,
        min_backoff_ms: 100,
        max_backoff_ms: 10_000,
        factor: 2,
        jitter_percent: 0,
        attempted_since_last_tick: true, // attempts happened
        phase: Phase::Startup { initial_tx: Some(tx), accumulated_errors: Vec::new() },
    };

    let (_next, actions) = sup.handle(Event::BackoffTick);

    // Should emit a NotifyInitial with Err
    assert!(actions.iter().any(|a| matches!(a, Action::NotifyInitial { result: Err(_), .. })));
}

#[test]
fn success_transition_enters_active_and_schedules_tick() {
    let providers = vec![ProviderState::Idle, ProviderState::Idle];
    let sup = Supervisor {
        providers,
        provider_instruments: vec![Vec::new(), Vec::new()],
        provider_allow: vec![std::collections::HashSet::new(), std::collections::HashSet::new()],
        required_symbols: std::collections::HashSet::new(),
        start_index: 0,
        backoff_ms: 100,
        min_backoff_ms: 100,
        max_backoff_ms: 10_000,
        factor: 2,
        jitter_percent: 0,
        attempted_since_last_tick: false,
        phase: Phase::Running,
    };

    // Success with empty symbol set
    let empty_syms: Arc<[borsa::core::Symbol]> = Arc::from(Vec::<borsa::core::Symbol>::new().into_boxed_slice());
    let (next, actions) = sup.handle(Event::ProviderStartSucceeded { id: 0, symbols: empty_syms });

    assert!(matches!(next.providers[0], ProviderState::Active{ .. }));
    assert!(actions.iter().any(|a| matches!(a, Action::ScheduleBackoffTick { .. })));
}


