use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use borsa_core::{BorsaError, Instrument, Symbol};
use tokio::sync::oneshot;

use super::error::collapse_stream_errors;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderState {
    Idle,
    /// Set on BackoffTick when clearing cooldowns; next success may reset backoff
    IdleFromCooldown,
    Active { session_meta: SessionMeta, symbols: Arc<[Symbol]> },
    InCooldown { failed_at: Instant },
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
    RequestStart { id: usize, instruments: Vec<Instrument> },
    StopSession { id: usize },
    StopAll,
    AwaitAll,
    NotifyInitial { tx: oneshot::Sender<Result<(), BorsaError>>, result: Result<(), BorsaError> },
    ScheduleBackoffTick { delay_ms: u64 },
    PreemptSessions { provider_ids: Vec<usize> },
}

#[derive(Debug)]
pub struct Supervisor {
    pub providers: Vec<ProviderState>,
    pub provider_instruments: Vec<Vec<Instrument>>, // aligned by provider
    pub provider_allow: Vec<HashSet<Symbol>>, // aligned by provider
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
    pub jitter_percent: u32,

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
                
                let from_cooldown = matches!(self.providers[id], ProviderState::IdleFromCooldown);
                self.providers[id] = ProviderState::Active { session_meta: SessionMeta::default(), symbols: Arc::clone(&symbols) };
                if from_cooldown { self.backoff_ms = self.min_backoff_ms; }
                self.start_index = (id + 1) % self.providers.len();
                self.scan_cursor = self.start_index;
                self.round_exhausted = false;
                let mut actions = Vec::new();
                if let Some(tx) = initial_tx { actions.push(Action::NotifyInitial { tx, result: Ok(()) }); }
                let lower_ids = self.compute_lower_priority_overlaps(id, &symbols);
                if !lower_ids.is_empty() { actions.push(Action::PreemptSessions { provider_ids: lower_ids }); }
                actions.push(Action::ScheduleBackoffTick { delay_ms: self.current_delay_ms() });
                (Self { phase: Phase::Running, ..self }, actions)
            }
            (Phase::Running, Event::ProviderStartSucceeded { id, symbols }) => {
                let from_cooldown = matches!(self.providers[id], ProviderState::IdleFromCooldown);
                self.providers[id] = ProviderState::Active { session_meta: SessionMeta::default(), symbols: Arc::clone(&symbols) };
                let mut actions = Vec::new();
                if from_cooldown { self.backoff_ms = self.min_backoff_ms; }
                self.start_index = (id + 1) % self.providers.len();
                self.scan_cursor = self.start_index;
                self.round_exhausted = false;
                let lower_ids = self.compute_lower_priority_overlaps(id, &symbols);
                if !lower_ids.is_empty() { actions.push(Action::PreemptSessions { provider_ids: lower_ids }); }
                actions.push(Action::ScheduleBackoffTick { delay_ms: self.current_delay_ms() });
                (self, actions)
            }
            (Phase::Startup { mut initial_tx, mut accumulated_errors }, Event::ProviderStartFailed { id, error }) => {
                accumulated_errors.push(error);

                // Mark the failed provider as being in cooldown
                self.providers[id] = ProviderState::InCooldown { failed_at: std::time::Instant::now() };
                let next_cursor = (id + 1) % self.providers.len();
                self.scan_cursor = next_cursor;
                if next_cursor == self.start_index { 
                    self.round_exhausted = true; 
                }
                if !self.has_any_active() && self.round_exhausted {
                    if let Some(tx) = initial_tx.take() {
                        return (Self { phase: Phase::Terminated, ..self }, vec![Action::NotifyInitial { tx, result: Err(collapse_stream_errors(accumulated_errors)) }]);
                    }
                }
                (Self { phase: Phase::Startup { initial_tx, accumulated_errors }, ..self }, Vec::new())
            }
            (phase @ Phase::Running, Event::ProviderStartFailed { id, .. }) => {
                self.providers[id] = ProviderState::InCooldown { failed_at: std::time::Instant::now() };
                let next_cursor = (id + 1) % self.providers.len();
                self.scan_cursor = next_cursor;
                if next_cursor == self.start_index { self.round_exhausted = true; }
                (Self { phase, ..self }, Vec::new())
            }
            (phase, Event::SessionEnded { id, .. }) => {
                self.providers[id] = ProviderState::InCooldown { failed_at: Instant::now() };
                (Self { phase, ..self }, Vec::new())
            }
            (phase, Event::BackoffTick) => {
                for p in &mut self.providers {
                    if matches!(p, ProviderState::InCooldown { .. }) {
                        *p = ProviderState::IdleFromCooldown;
                    }
                }
                if self.attempted_since_last_tick {
                    if !self.has_any_active() {
                        // ONLY terminate if we've tried all providers
                        if self.round_exhausted {
                            if let Phase::Startup { initial_tx: Some(tx), accumulated_errors } = phase {
                                return (Self { phase: Phase::Terminated, ..self }, 
                                        vec![Action::NotifyInitial { tx, result: Err(collapse_stream_errors(accumulated_errors)) }]);
                            }
                        }
                        self.backoff_ms = self.backoff_ms.saturating_mul(self.factor.into()).min(self.max_backoff_ms);
                        self.start_index = 0;
                    } else {
                        self.backoff_ms = self.backoff_ms.saturating_mul(self.factor.into()).min(self.max_backoff_ms);
                    }
                }
                self.attempted_since_last_tick = false;
                self.scan_cursor = self.start_index;
                self.round_exhausted = false;
                let delay = self.current_delay_ms();
                (Self { phase, ..self }, vec![Action::ScheduleBackoffTick { delay_ms: delay }])
            }
            (_, Event::Shutdown) | (_, Event::DownstreamClosed) => {
                (Self { phase: Phase::ShuttingDown, ..self }, vec![Action::StopAll, Action::AwaitAll])
            }
            (Phase::Terminated, _) => (self, Vec::new()),
            (Phase::ShuttingDown, _) => (self, Vec::new()),
        }
    }

    fn compute_coverage_count(&self, sym: &Symbol) -> usize {
        self.providers
            .iter()
            .filter_map(|p| match p {
                ProviderState::Active { symbols, .. } => Some(symbols),
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
                .filter(|inst| {
                    let sym = inst.symbol();
                    if !allow_set.contains(sym) || !self.required_symbols.contains(sym) {
                        return false;
                    }
                    let already_covered = self.compute_coverage_count(sym) > 0;
                    if !already_covered {
                        return true;
                    }
                    !self.provider_has_symbol_before(id, sym)
                })
                .cloned()
                .collect(),
            _ => Vec::new(),
        }
    }

    pub fn should_attempt_starts(&self) -> bool {
        if self.round_exhausted { return false; }
        for (i, state) in self.providers.iter().enumerate() {
            match state {
                ProviderState::Idle | ProviderState::IdleFromCooldown => {
                    if self.providers_can_stream.get(i).copied().unwrap_or(false)
                        && !self.compute_needed_instruments_for(i).is_empty()
                    {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    pub fn compute_needed_starts(&self) -> Vec<Action> {


        let len = self.providers.len();
        if len == 0 || self.round_exhausted { 
            return Vec::new(); 
        }
        let mut i = self.scan_cursor % len;
        let start = self.start_index % len;
        let mut first = true;
        loop {
            match self.providers.get(i) {
                Some(ProviderState::Idle) | Some(ProviderState::IdleFromCooldown) => {
                    if self.providers_can_stream.get(i).copied().unwrap_or(false) {
                        let instruments = self.compute_needed_instruments_for(i);
                        if !instruments.is_empty() {
                            return vec![Action::RequestStart { id: i, instruments }];
                        }
                    }
                }
                _ => {}
            }
            if !first && i == start { break; }
            first = false;
            i = (i + 1) % len;
        }
        Vec::new()
    }

    pub fn has_any_active(&self) -> bool {
        self.providers.iter().any(|p| matches!(p, ProviderState::Active { .. }))
    }

    pub fn compute_lower_priority_overlaps(&self, higher_id: usize, symbols: &[Symbol]) -> Vec<usize> {
        let mut to_preempt: Vec<usize> = Vec::new();
        for j in (higher_id + 1)..self.providers.len() {
            if let ProviderState::Active { symbols: active_symbols, .. } = &self.providers[j] {
                let overlaps = active_symbols.iter().any(|s| symbols.iter().any(|t| t == s));
                if overlaps {
                    to_preempt.push(j);
                }
            }
        }
        to_preempt
    }

    pub fn current_delay_ms(&self) -> u64 { self.backoff_ms }
}


