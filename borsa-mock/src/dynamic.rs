use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{Mutex, mpsc, oneshot};

use borsa_core::connector::{BorsaConnector, HistoryProvider, QuoteProvider, StreamProvider};
use borsa_core::{
    AssetKind, BorsaError, HistoryRequest, HistoryResponse, Instrument, Interval, Quote,
    QuoteUpdate, Symbol,
};

/// Instruction for how a method should behave for a given input.
#[derive(Clone)]
pub enum MockBehavior<T> {
    /// Return the provided value immediately.
    Return(T),
    /// Fail immediately with the provided error.
    Fail(BorsaError),
    /// Hang indefinitely (simulate a timeout).
    Hang,
}

/// Instruction for how a stream should behave for a given provider name.
#[derive(Clone)]
pub enum StreamBehavior {
    /// Start a stream and send these updates (filtered by requested symbols).
    Success(Vec<QuoteUpdate>),
    /// Fail the `stream_quotes` call immediately.
    Fail(BorsaError),
    /// Hang the `stream_quotes` call (simulate a network stall during connect).
    Hang,
    /// Start a stream that accepts external updates via controller `push_update`.
    Manual,
}

struct StreamController {
    behavior: StreamBehavior,
    next_session_id: u64,
    kill_switches: HashMap<u64, oneshot::Sender<()>>, // remote kill switches per active session
    manual_txs: HashMap<u64, mpsc::Sender<QuoteUpdate>>, // inbound updates per Manual session
}

impl StreamController {
    fn new(behavior: StreamBehavior) -> Self {
        Self {
            behavior,
            next_session_id: 0,
            kill_switches: HashMap::new(),
            manual_txs: HashMap::new(),
        }
    }
}

#[derive(Default)]
struct InternalState {
    quote_rules: HashMap<Symbol, MockBehavior<Quote>>,
    history_rules: HashMap<Symbol, MockBehavior<HistoryResponse>>,
    // Prediction support (variant-aware behavior keyed by OutcomeID)
    quote_rules_prediction: HashMap<String, MockBehavior<Quote>>,
    history_rules_prediction: HashMap<String, MockBehavior<HistoryResponse>>,
    stream_requests: HashMap<&'static str, Vec<Vec<Instrument>>>,
    stream_controllers: HashMap<&'static str, StreamController>,
}

/// Controller handle used by tests to drive the dynamic mock from the outside.
pub struct DynamicMockController {
    state: Arc<Mutex<InternalState>>,
}

impl DynamicMockController {
    /// Set the behavior for `quote` calls for a specific symbol.
    pub async fn set_quote_behavior(&self, symbol: Symbol, behavior: MockBehavior<Quote>) {
        let mut guard = self.state.lock().await;
        guard.quote_rules.insert(symbol, behavior);
    }

    /// Set the behavior for `history` calls for a specific symbol.
    pub async fn set_history_behavior(
        &self,
        symbol: Symbol,
        behavior: MockBehavior<HistoryResponse>,
    ) {
        let mut guard = self.state.lock().await;
        guard.history_rules.insert(symbol, behavior);
    }

    /// Set the behavior for `quote` calls for a specific prediction outcome.
    pub async fn set_prediction_quote_behavior(
        &self,
        outcome: String,
        behavior: MockBehavior<Quote>,
    ) {
        let mut guard = self.state.lock().await;
        guard.quote_rules_prediction.insert(outcome, behavior);
    }

    /// Set the behavior for `history` calls for a specific prediction outcome.
    pub async fn set_prediction_history_behavior(
        &self,
        outcome: String,
        behavior: MockBehavior<HistoryResponse>,
    ) {
        let mut guard = self.state.lock().await;
        guard.history_rules_prediction.insert(outcome, behavior);
    }

    /// Set the behavior for a provider's stream session.
    pub async fn set_stream_behavior(&self, provider_name: &'static str, behavior: StreamBehavior) {
        let mut guard = self.state.lock().await;
        match guard.stream_controllers.get_mut(provider_name) {
            Some(ctrl) => ctrl.behavior = behavior,
            None => {
                guard
                    .stream_controllers
                    .insert(provider_name, StreamController::new(behavior));
            }
        }
    }

    /// Remotely kill an active stream for the given provider name.
    pub async fn fail_stream(&self, provider_name: &'static str) {
        let mut guard = self.state.lock().await;
        if let Some(ctrl) = guard.stream_controllers.get_mut(provider_name) {
            let mut kill_switches = std::mem::take(&mut ctrl.kill_switches);
            for (_, tx) in kill_switches.drain() {
                let _ = tx.send(());
            }
            ctrl.manual_txs.clear();
        }
    }

    /// Push a single update into an active Manual stream.
    ///
    /// Returns `true` if the update was queued, `false` if no Manual session is active
    /// or the channel is closed.
    pub async fn push_update(&self, provider_name: &'static str, update: QuoteUpdate) -> bool {
        let sessions = {
            let guard = self.state.lock().await;
            guard.stream_controllers.get(provider_name).map(|c| {
                c.manual_txs
                    .iter()
                    .map(|(id, tx)| (*id, tx.clone()))
                    .collect::<Vec<_>>()
            })
        };
        let Some(sessions) = sessions else {
            return false;
        };
        if sessions.is_empty() {
            return false;
        }

        let mut any_sent = false;
        let mut failed_ids: Vec<u64> = Vec::new();
        for (id, tx) in sessions {
            match tx.send(update.clone()).await {
                Ok(()) => any_sent = true,
                Err(_) => failed_ids.push(id),
            }
        }

        if !failed_ids.is_empty() {
            let mut guard = self.state.lock().await;
            if let Some(ctrl) = guard.stream_controllers.get_mut(provider_name) {
                for id in failed_ids {
                    ctrl.manual_txs.remove(&id);
                    ctrl.kill_switches.remove(&id);
                }
            }
        }

        any_sent
    }

    /// Return a copy of the request log for the given provider name.
    pub async fn get_stream_requests(&self, provider_name: &'static str) -> Vec<Vec<Instrument>> {
        let guard = self.state.lock().await;
        guard
            .stream_requests
            .get(provider_name)
            .cloned()
            .unwrap_or_default()
    }

    /// Clear all configured behaviors and request logs.
    pub async fn clear_all_behaviors(&self) {
        let mut guard = self.state.lock().await;
        guard.quote_rules.clear();
        guard.history_rules.clear();
        guard.stream_requests.clear();
        guard.stream_controllers.clear();
    }
}

/// A connector that defers all behavior to an external controller.
pub struct DynamicMockConnector {
    name: &'static str,
    state: Arc<Mutex<InternalState>>,
}

impl DynamicMockConnector {
    /// Create a new dynamic mock connector and its controller.
    #[must_use]
    pub fn new_with_controller(
        name: &'static str,
    ) -> (Arc<dyn BorsaConnector>, DynamicMockController) {
        let state = Arc::new(Mutex::new(InternalState::default()));
        let controller = DynamicMockController {
            state: Arc::clone(&state),
        };
        let me = Arc::new(Self { name, state });
        (me as Arc<dyn BorsaConnector>, controller)
    }

    /// Create a new dynamic mock connector with an initial stream behavior preinstalled.
    #[must_use]
    pub fn new_with_controller_and_behavior(
        name: &'static str,
        behavior: StreamBehavior,
    ) -> (Arc<dyn BorsaConnector>, DynamicMockController) {
        let mut initial = InternalState::default();
        initial
            .stream_controllers
            .insert(name, StreamController::new(behavior));
        let state = Arc::new(Mutex::new(initial));
        let controller = DynamicMockController {
            state: Arc::clone(&state),
        };
        let me = Arc::new(Self { name, state });
        (me as Arc<dyn BorsaConnector>, controller)
    }
}

fn require_security_symbol(inst: &Instrument) -> Result<&Symbol, BorsaError> {
    match inst.id() {
        borsa_core::IdentifierScheme::Security(sec) => Ok(&sec.symbol),
        borsa_core::IdentifierScheme::Prediction(_) => Err(BorsaError::unsupported(
            "instrument scheme (mock/security-only)",
        )),
    }
}

#[async_trait]
impl BorsaConnector for DynamicMockConnector {
    fn name(&self) -> &'static str {
        self.name
    }

    fn vendor(&self) -> &'static str {
        "DynamicMock"
    }

    fn supports_kind(&self, _kind: AssetKind) -> bool {
        true
    }

    fn as_quote_provider(&self) -> Option<&dyn QuoteProvider> {
        Some(self as &dyn QuoteProvider)
    }

    fn as_history_provider(&self) -> Option<&dyn HistoryProvider> {
        Some(self as &dyn HistoryProvider)
    }

    fn as_stream_provider(&self) -> Option<&dyn StreamProvider> {
        Some(self as &dyn StreamProvider)
    }
}

#[async_trait]
impl QuoteProvider for DynamicMockConnector {
    async fn quote(&self, instrument: &Instrument) -> Result<Quote, BorsaError> {
        // Acquire behavior snapshot without holding the lock across await points
        let behavior = {
            let guard = self.state.lock().await;
            match instrument.id() {
                borsa_core::IdentifierScheme::Security(sec) => {
                    guard.quote_rules.get(&sec.symbol).cloned()
                }
                borsa_core::IdentifierScheme::Prediction(pred) => {
                    let key = pred.outcome_id.as_ref().to_string();
                    guard.quote_rules_prediction.get(&key).cloned()
                }
            }
        };

        match behavior {
            Some(MockBehavior::Return(q)) => Ok(q),
            Some(MockBehavior::Fail(e)) => Err(e),
            Some(MockBehavior::Hang) => {
                std::future::pending::<()>().await;
                unreachable!()
            }
            None => Err(BorsaError::unsupported("quote")),
        }
    }
}

#[async_trait]
impl HistoryProvider for DynamicMockConnector {
    async fn history(
        &self,
        instrument: &Instrument,
        _req: HistoryRequest,
    ) -> Result<HistoryResponse, BorsaError> {
        let behavior = {
            let guard = self.state.lock().await;
            match instrument.id() {
                borsa_core::IdentifierScheme::Security(sec) => {
                    guard.history_rules.get(&sec.symbol).cloned()
                }
                borsa_core::IdentifierScheme::Prediction(pred) => {
                    let key = pred.outcome_id.as_ref().to_string();
                    guard.history_rules_prediction.get(&key).cloned()
                }
            }
        };

        match behavior {
            Some(MockBehavior::Return(resp)) => Ok(resp),
            Some(MockBehavior::Fail(e)) => Err(e),
            Some(MockBehavior::Hang) => {
                std::future::pending::<()>().await;
                unreachable!()
            }
            None => Err(BorsaError::unsupported("history")),
        }
    }

    fn supported_history_intervals(&self, _kind: AssetKind) -> &'static [Interval] {
        const ONLY_D1: &[Interval] = &[Interval::D1];
        ONLY_D1
    }
}

impl DynamicMockConnector {
    async fn stream_manual_behavior(
        &self,
        instruments: &[Instrument],
    ) -> Result<
        (
            borsa_core::stream::StreamHandle,
            mpsc::Receiver<QuoteUpdate>,
        ),
        BorsaError,
    > {
        // Filter set
        let allow: std::collections::HashSet<Symbol> = instruments
            .iter()
            .map(require_security_symbol)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .cloned()
            .collect();

        let (tx, rx) = mpsc::channel::<QuoteUpdate>(1024);
        let (stop_tx, mut stop_rx) = oneshot::channel::<()>();
        let (kill_tx, mut kill_rx) = oneshot::channel::<()>();
        let (in_tx, mut in_rx) = mpsc::channel::<QuoteUpdate>(1024);

        // Publish kill switches and manual sender for remote control
        let session_id = {
            let mut guard = self.state.lock().await;
            let entry = guard
                .stream_controllers
                .entry(self.name)
                .or_insert_with(|| StreamController::new(StreamBehavior::Manual));
            entry.behavior = StreamBehavior::Manual;
            let sid = entry.next_session_id;
            entry.next_session_id += 1;
            entry.kill_switches.insert(sid, kill_tx);
            entry.manual_txs.insert(sid, in_tx.clone());
            drop(guard);
            sid
        };

        let state = Arc::clone(&self.state);
        let provider_name = self.name;

        let join = tokio::spawn(async move {
            loop {
                tokio::select! {
                    biased;
                    _ = &mut stop_rx => { break; }
                    _ = &mut kill_rx => { break; }
                    maybe_u = in_rx.recv() => {
                        if let Some(u) = maybe_u {
                            if !allow.is_empty() && !allow.contains(&u.symbol) {
                                continue;
                            }
                            // Forward; drop if downstream closed
                            if tx.send(u).await.is_err() { break; }
                        } else {
                            // Controller dropped manual sender; wait for stop/kill
                            // to avoid busy loop
                            tokio::select! {
                                _ = &mut stop_rx => {}
                                _ = &mut kill_rx => {}
                            }
                            break;
                        }
                    }
                }
            }

            let mut guard = state.lock().await;
            if let Some(ctrl) = guard.stream_controllers.get_mut(provider_name) {
                ctrl.manual_txs.remove(&session_id);
                ctrl.kill_switches.remove(&session_id);
            }
        });

        Ok((borsa_core::stream::StreamHandle::new(join, stop_tx), rx))
    }

    async fn stream_success_behavior(
        &self,
        instruments: &[Instrument],
        updates: Vec<QuoteUpdate>,
    ) -> Result<
        (
            borsa_core::stream::StreamHandle,
            mpsc::Receiver<QuoteUpdate>,
        ),
        BorsaError,
    > {
        // Filter set
        let allow: std::collections::HashSet<Symbol> = instruments
            .iter()
            .map(require_security_symbol)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .cloned()
            .collect();

        let (tx, rx) = mpsc::channel::<QuoteUpdate>(1024);
        let (stop_tx, mut stop_rx) = oneshot::channel::<()>();
        let (kill_tx, mut kill_rx) = oneshot::channel::<()>();

        // Publish kill switch for remote failure
        let session_id = {
            let mut guard = self.state.lock().await;
            let entry = guard
                .stream_controllers
                .entry(self.name)
                .or_insert_with(|| StreamController::new(StreamBehavior::Success(Vec::new())));
            entry.behavior = StreamBehavior::Success(Vec::new());
            let sid = entry.next_session_id;
            entry.next_session_id += 1;
            entry.kill_switches.insert(sid, kill_tx);
            entry.manual_txs.clear();
            drop(guard);
            sid
        };

        let state = Arc::clone(&self.state);
        let provider_name = self.name;

        let join = tokio::spawn(async move {
            // Send scripted updates, respecting allow filter, until stopped/killed.
            for u in updates {
                if !allow.is_empty() && !allow.contains(&u.symbol) {
                    continue;
                }
                tokio::select! {
                    biased;
                    _ = &mut stop_rx => { return; }
                    _ = &mut kill_rx => { return; }
                    res = tx.send(u) => {
                        if res.is_err() {
                            return;
                        }
                    }
                }
            }
            // Keep the channel open until a stop/kill arrives, then drop sender
            tokio::select! {
                _ = &mut stop_rx => {}
                _ = &mut kill_rx => {}
            }

            let mut guard = state.lock().await;
            if let Some(ctrl) = guard.stream_controllers.get_mut(provider_name) {
                ctrl.kill_switches.remove(&session_id);
            }
        });

        Ok((borsa_core::stream::StreamHandle::new(join, stop_tx), rx))
    }
}

#[async_trait]
impl StreamProvider for DynamicMockConnector {
    async fn stream_quotes(
        &self,
        instruments: &[Instrument],
    ) -> Result<
        (
            borsa_core::stream::StreamHandle,
            mpsc::Receiver<QuoteUpdate>,
        ),
        BorsaError,
    > {
        // Log the request
        {
            let mut guard = self.state.lock().await;
            guard
                .stream_requests
                .entry(self.name)
                .or_default()
                .push(instruments.to_vec());
        }

        // Fetch current behavior for this provider
        let behavior = {
            let guard = self.state.lock().await;
            guard
                .stream_controllers
                .get(self.name)
                .map(|c| c.behavior.clone())
        };

        match behavior {
            Some(StreamBehavior::Fail(e)) => Err(e),
            Some(StreamBehavior::Hang) => {
                std::future::pending::<()>().await;
                unreachable!()
            }
            Some(StreamBehavior::Manual) => self.stream_manual_behavior(instruments).await,
            Some(StreamBehavior::Success(updates)) => {
                self.stream_success_behavior(instruments, updates).await
            }
            None => Err(BorsaError::unsupported("stream_quotes")),
        }
    }
}
