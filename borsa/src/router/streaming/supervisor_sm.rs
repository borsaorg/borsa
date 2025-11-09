use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use borsa_core::{BorsaError, Instrument, Symbol};
use tokio::sync::oneshot;

use super::error::collapse_stream_errors;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderState {
    Idle,
    /// Set on `BackoffTick` when clearing cooldowns; next success may reset backoff
    IdleFromCooldown,
    /// A start request has been issued for this provider and is in-flight
    Connecting {
        symbols: Arc<[Symbol]>,
    },
    Active {
        session_meta: SessionMeta,
        symbols: Arc<[Symbol]>,
    },
    InCooldown {
        failed_at: Instant,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionMeta {}

#[derive(Debug)]
pub enum Phase {
    Startup {
        initial_tx: Option<oneshot::Sender<Result<(), BorsaError>>>,
        accumulated_errors: Vec<BorsaError>,
    },
    Running,
    ShuttingDown,
    Terminated,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    ProviderStartSucceeded { id: usize, symbols: Arc<[Symbol]> },
    ProviderStartFailed { id: usize, error: BorsaError },
    SessionEnded { id: usize, symbols: Arc<[Symbol]> },
    BackoffTick,
    DownstreamClosed,
    Shutdown,
}

#[derive(Debug)]
pub enum Action {
    RequestStart {
        id: usize,
        instruments: Vec<Instrument>,
    },
    StopAll,
    AwaitAll,
    NotifyInitial {
        tx: oneshot::Sender<Result<(), BorsaError>>,
        result: Result<(), BorsaError>,
    },
    ScheduleBackoffTick {
        delay_ms: u64,
    },
    PreemptSessions {
        provider_ids: Vec<usize>,
    },
}

#[derive(Debug)]
pub struct Supervisor {
    pub providers: Vec<ProviderState>,
    pub provider_instruments: Vec<Vec<Instrument>>, // aligned by provider
    pub provider_allow: Vec<HashSet<Symbol>>,       // aligned by provider
    pub required_symbols: HashSet<Symbol>,
    /// Whether each provider supports streaming (driver provides this)
    pub providers_can_stream: Vec<bool>,

    pub start_index: usize,
    /// Next provider to consider during this round
    pub scan_cursor: usize,
    /// Whether we've completed a full round scan since the last tick
    pub round_exhausted: bool,
    pub backoff_ms: u64,
    pub min_backoff_ms: u64,
    pub max_backoff_ms: u64,
    pub factor: u32,

    pub attempted_since_last_tick: bool,
    pub phase: Phase,
}

impl Supervisor {
    pub fn handle(self, event: Event) -> (Self, Vec<Action>) {
        let (mut next, mut actions) = self.transition_for_event(event);
        if next.should_attempt_starts() {
            let mut reqs = next.compute_needed_starts();
            if !reqs.is_empty() {
                next.attempted_since_last_tick = true;
                actions.append(&mut reqs);
            }
        }
        (next, actions)
    }

    fn transition_for_event(mut self, event: Event) -> (Self, Vec<Action>) {
        // Take ownership of the current phase without moving other fields
        let prev_phase = std::mem::replace(&mut self.phase, Phase::Running);
        match (prev_phase, event) {
            (Phase::Startup { initial_tx, .. }, Event::ProviderStartSucceeded { id, symbols }) => {
                let mut actions = self.handle_provider_activated(id, &symbols);
                if let Some(tx) = initial_tx {
                    actions.insert(0, Action::NotifyInitial { tx, result: Ok(()) });
                }
                (
                    Self {
                        phase: Phase::Running,
                        ..self
                    },
                    actions,
                )
            }
            (Phase::Running, Event::ProviderStartSucceeded { id, symbols }) => {
                let actions = self.handle_provider_activated(id, &symbols);
                (self, actions)
            }
            (
                Phase::Startup {
                    initial_tx,
                    accumulated_errors,
                },
                Event::ProviderStartFailed { id, error },
            ) => self.handle_startup_failure(id, error, initial_tx, accumulated_errors),
            (phase @ Phase::Running, Event::ProviderStartFailed { id, .. }) => {
                self.advance_scan_cursor_for_failure(id);
                (Self { phase, ..self }, Vec::new())
            }
            (phase, Event::SessionEnded { id, .. }) => {
                self.providers[id] = ProviderState::InCooldown {
                    failed_at: Instant::now(),
                };
                (Self { phase, ..self }, Vec::new())
            }
            (phase, Event::BackoffTick) => self.handle_backoff_tick(phase),
            (_, Event::Shutdown | Event::DownstreamClosed) => (
                Self {
                    phase: Phase::ShuttingDown,
                    ..self
                },
                vec![Action::StopAll, Action::AwaitAll],
            ),
            (Phase::Terminated | Phase::ShuttingDown, _) => (self, Vec::new()),
        }
    }

    fn compute_coverage_count(&self, sym: &Symbol) -> usize {
        self.providers
            .iter()
            .filter_map(|p| match p {
                ProviderState::Active { symbols, .. } | ProviderState::Connecting { symbols } => {
                    Some(symbols)
                }
                _ => None,
            })
            .filter(|symbols| symbols.iter().any(|s| s == sym))
            .count()
    }

    fn provider_has_symbol_before(&self, provider_index: usize, sym: &Symbol) -> bool {
        self.providers.iter().enumerate().any(|(j, state)| {
            j < provider_index
                && match state {
                    ProviderState::Active { symbols, .. } => symbols.iter().any(|s2| s2 == sym),
                    ProviderState::Connecting { symbols } => symbols.iter().any(|s2| s2 == sym),
                    _ => false,
                }
        })
    }

    pub fn compute_needed_instruments_for(&self, id: usize) -> Vec<Instrument> {
        let provider_symbols = self.provider_allow.get(id);
        let provider_insts = self.provider_instruments.get(id);
        match (provider_symbols, provider_insts) {
            (Some(allow_set), Some(insts)) => insts
                .iter()
                .filter(|inst| self.should_include_instrument(id, inst, allow_set))
                .cloned()
                .collect(),
            _ => Vec::new(),
        }
    }

    pub fn should_attempt_starts(&self) -> bool {
        !self.round_exhausted && self.has_idle_providers_with_work()
    }

    pub fn compute_needed_starts(&mut self) -> Vec<Action> {
        let len = self.providers.len();
        if len == 0 || self.round_exhausted {
            return Vec::new();
        }
        let mut i = self.scan_cursor % len;
        let start = self.start_index % len;
        let mut first = true;
        let mut actions: Vec<Action> = Vec::new();
        loop {
            if let Some(state) = self.providers.get(i)
                && Self::is_provider_idle(state)
                && self.can_provider_stream(i)
            {
                let instruments = self.compute_needed_instruments_for(i);
                if !instruments.is_empty() {
                    // mark provider as connecting with the planned symbol set
                    let syms: Arc<[Symbol]> = Arc::from(
                        instruments
                            .iter()
                            .filter_map(|inst| match inst.id() {
                                borsa_core::IdentifierScheme::Security(sec) => {
                                    Some(sec.symbol.clone())
                                }
                                borsa_core::IdentifierScheme::Prediction(_) => None,
                            })
                            .collect::<Vec<_>>()
                            .into_boxed_slice(),
                    );
                    self.providers[i] = ProviderState::Connecting {
                        symbols: Arc::clone(&syms),
                    };
                    actions.push(Action::RequestStart { id: i, instruments });
                }
            }
            if !first && i == start {
                break;
            }
            first = false;
            i = (i + 1) % len;
        }
        actions
    }

    pub fn has_any_active(&self) -> bool {
        self.providers
            .iter()
            .any(|p| matches!(p, ProviderState::Active { .. }))
    }

    pub fn compute_lower_priority_overlaps(
        &self,
        higher_id: usize,
        symbols: &[Symbol],
    ) -> Vec<usize> {
        let mut to_preempt: Vec<usize> = Vec::new();
        for j in (higher_id + 1)..self.providers.len() {
            if let ProviderState::Active {
                symbols: active_symbols,
                ..
            } = &self.providers[j]
            {
                let overlaps = active_symbols
                    .iter()
                    .any(|s| symbols.iter().any(|t| t == s));
                if overlaps {
                    to_preempt.push(j);
                }
            }
        }
        to_preempt
    }

    pub const fn current_delay_ms(&self) -> u64 {
        self.backoff_ms
    }

    const fn is_provider_idle(state: &ProviderState) -> bool {
        matches!(state, ProviderState::Idle | ProviderState::IdleFromCooldown)
    }

    const fn is_provider_idle_from_cooldown(state: &ProviderState) -> bool {
        matches!(state, ProviderState::IdleFromCooldown)
    }

    fn can_provider_stream(&self, provider_id: usize) -> bool {
        self.providers_can_stream
            .get(provider_id)
            .copied()
            .unwrap_or(false)
    }

    fn provider_has_available_work(&self, provider_id: usize) -> bool {
        !self.compute_needed_instruments_for(provider_id).is_empty()
    }

    fn has_idle_providers_with_work(&self) -> bool {
        self.providers.iter().enumerate().any(|(i, state)| {
            Self::is_provider_idle(state)
                && self.can_provider_stream(i)
                && self.provider_has_available_work(i)
        })
    }

    fn should_include_instrument(
        &self,
        provider_id: usize,
        inst: &Instrument,
        allow_set: &HashSet<Symbol>,
    ) -> bool {
        let sym_opt = match inst.id() {
            borsa_core::IdentifierScheme::Security(sec) => Some(&sec.symbol),
            borsa_core::IdentifierScheme::Prediction(_) => None,
        };
        let Some(sym) = sym_opt else {
            return false;
        };

        if !allow_set.contains(sym) || !self.required_symbols.contains(sym) {
            return false;
        }

        let already_covered = self.compute_coverage_count(sym) > 0;
        if !already_covered {
            return true;
        }

        !self.provider_has_symbol_before(provider_id, sym)
    }

    fn handle_provider_activated(&mut self, id: usize, symbols: &Arc<[Symbol]>) -> Vec<Action> {
        let from_cooldown = Self::is_provider_idle_from_cooldown(&self.providers[id]);
        self.providers[id] = ProviderState::Active {
            session_meta: SessionMeta::default(),
            symbols: Arc::clone(symbols),
        };

        if from_cooldown {
            self.backoff_ms = self.min_backoff_ms;
        }

        self.start_index = (id + 1) % self.providers.len();
        self.scan_cursor = self.start_index;
        self.round_exhausted = false;

        let mut actions = Vec::new();
        let lower_ids = self.compute_lower_priority_overlaps(id, symbols);
        if !lower_ids.is_empty() {
            actions.push(Action::PreemptSessions {
                provider_ids: lower_ids,
            });
        }
        actions.push(Action::ScheduleBackoffTick {
            delay_ms: self.current_delay_ms(),
        });
        actions
    }

    fn advance_scan_cursor_for_failure(&mut self, id: usize) {
        self.providers[id] = ProviderState::InCooldown {
            failed_at: Instant::now(),
        };
        let next_cursor = (id + 1) % self.providers.len();
        self.scan_cursor = next_cursor;
        if next_cursor == self.start_index {
            self.round_exhausted = true;
        }
    }

    fn should_terminate_startup(&self) -> bool {
        !self.has_any_active() && self.round_exhausted
    }

    fn handle_startup_failure(
        mut self,
        id: usize,
        error: BorsaError,
        initial_tx: Option<oneshot::Sender<Result<(), BorsaError>>>,
        mut accumulated_errors: Vec<BorsaError>,
    ) -> (Self, Vec<Action>) {
        accumulated_errors.push(error);
        self.advance_scan_cursor_for_failure(id);

        if self.should_terminate_startup()
            && let Some(tx) = initial_tx
        {
            return (
                Self {
                    phase: Phase::Terminated,
                    ..self
                },
                vec![Action::NotifyInitial {
                    tx,
                    result: Err(collapse_stream_errors(accumulated_errors)),
                }],
            );
        }

        (
            Self {
                phase: Phase::Startup {
                    initial_tx,
                    accumulated_errors,
                },
                ..self
            },
            Vec::new(),
        )
    }

    fn handle_backoff_tick(mut self, phase: Phase) -> (Self, Vec<Action>) {
        for p in &mut self.providers {
            if matches!(p, ProviderState::InCooldown { .. }) {
                *p = ProviderState::IdleFromCooldown;
            }
        }

        if self.attempted_since_last_tick {
            if self.has_any_active() {
                self.increase_backoff();
            } else {
                if self.round_exhausted
                    && let Phase::Startup {
                        initial_tx: Some(tx),
                        accumulated_errors,
                    } = phase
                {
                    return (
                        Self {
                            phase: Phase::Terminated,
                            ..self
                        },
                        vec![Action::NotifyInitial {
                            tx,
                            result: Err(collapse_stream_errors(accumulated_errors)),
                        }],
                    );
                }
                self.increase_backoff();
                self.start_index = 0;
            }
        }

        self.attempted_since_last_tick = false;
        self.scan_cursor = self.start_index;
        self.round_exhausted = false;
        let delay = self.current_delay_ms();

        (
            Self { phase, ..self },
            vec![Action::ScheduleBackoffTick { delay_ms: delay }],
        )
    }

    fn increase_backoff(&mut self) {
        self.backoff_ms = self
            .backoff_ms
            .saturating_mul(self.factor.into())
            .min(self.max_backoff_ms);
    }
}
