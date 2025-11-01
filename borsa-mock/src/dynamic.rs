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
    kill_switch: Option<oneshot::Sender<()>>, // remote kill switch
    manual_tx: Option<mpsc::Sender<QuoteUpdate>>, // inbound updates for Manual behavior
}

impl StreamController {
    const fn new(behavior: StreamBehavior) -> Self {
        Self {
            behavior,
            kill_switch: None,
            manual_tx: None,
        }
    }
}

#[derive(Default)]
struct InternalState {
    quote_rules: HashMap<Symbol, MockBehavior<Quote>>,
    history_rules: HashMap<Symbol, MockBehavior<HistoryResponse>>,
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
        if let Some(ctrl) = guard.stream_controllers.get_mut(provider_name)
            && let Some(tx) = ctrl.kill_switch.take()
        {
            let _ = tx.send(());
        }
    }

    /// Push a single update into an active Manual stream.
    ///
    /// Returns `true` if the update was queued, `false` if no Manual session is active
    /// or the channel is closed.
    pub async fn push_update(&self, provider_name: &'static str, update: QuoteUpdate) -> bool {
        // Extract a sender clone without holding the lock across await
        let tx_opt = {
            let mut guard = self.state.lock().await;
            guard
                .stream_controllers
                .get_mut(provider_name)
                .and_then(|c| c.manual_tx.clone())
        };
        if let Some(tx) = tx_opt {
            tx.send(update).await.is_ok()
        } else {
            false
        }
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
        let symbol = instrument.symbol();
        // Acquire behavior snapshot without holding the lock across await points
        let behavior = {
            let guard = self.state.lock().await;
            guard.quote_rules.get(symbol).cloned()
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
        let symbol = instrument.symbol();
        let behavior = {
            let guard = self.state.lock().await;
            guard.history_rules.get(symbol).cloned()
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
            Some(StreamBehavior::Manual) => {
                // Filter set
                let allow: std::collections::HashSet<Symbol> =
                    instruments.iter().map(|i| i.symbol().clone()).collect();

                let (tx, rx) = mpsc::channel::<QuoteUpdate>(1024);
                let (stop_tx, mut stop_rx) = oneshot::channel::<()>();
                let (kill_tx, mut kill_rx) = oneshot::channel::<()>();
                let (in_tx, mut in_rx) = mpsc::channel::<QuoteUpdate>(1024);

                // Publish kill switch and manual sender for remote control
                {
                    let mut guard = self.state.lock().await;
                    let entry = guard
                        .stream_controllers
                        .entry(self.name)
                        .or_insert_with(|| StreamController::new(StreamBehavior::Manual));
                    entry.kill_switch = Some(kill_tx);
                    entry.manual_tx = Some(in_tx.clone());
                }

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
                });

                Ok((borsa_core::stream::StreamHandle::new(join, stop_tx), rx))
            }
            Some(StreamBehavior::Success(updates)) => {
                // Filter set
                let allow: std::collections::HashSet<Symbol> =
                    instruments.iter().map(|i| i.symbol().clone()).collect();

                let (tx, rx) = mpsc::channel::<QuoteUpdate>(1024);
                let (stop_tx, mut stop_rx) = oneshot::channel::<()>();
                let (kill_tx, mut kill_rx) = oneshot::channel::<()>();

                // Publish kill switch for remote failure
                {
                    let mut guard = self.state.lock().await;
                    let entry = guard
                        .stream_controllers
                        .entry(self.name)
                        .or_insert_with(|| {
                            StreamController::new(StreamBehavior::Success(Vec::new()))
                        });
                    entry.kill_switch = Some(kill_tx);
                    // ensure manual_tx cleared for non-Manual behaviors
                    entry.manual_tx = None;
                }

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
                });

                Ok((borsa_core::stream::StreamHandle::new(join, stop_tx), rx))
            }
            None => Err(BorsaError::unsupported("stream_quotes")),
        }
    }
}
