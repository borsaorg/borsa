pub mod backoff;
pub mod controller;
pub mod error;
pub mod filters;
pub mod planner;
pub mod session;
pub mod supervisor_sm;

pub use controller::{KindSupervisorParams, spawn_kind_supervisor};
pub use error::collapse_stream_errors;
pub use planner::EligibleStreamProviders;

use borsa_core::{
    BorsaConnector, BorsaError, CandleUpdate, IdentifierScheme, Instrument, Interval, OptionUpdate,
    QuoteUpdate, Symbol, stream::StreamHandle,
};
use chrono::{DateTime, Utc};
use tokio::sync::mpsc;

/// Common surface for streaming updates used by session filters and gating.
pub trait StreamableUpdate: Send + 'static {
    /// The unique key for gating and session assignment (typically the symbol).
    fn stream_symbol(&self) -> &Symbol;
    /// Update timestamp for monotonic enforcement.
    fn stream_ts(&self) -> DateTime<Utc>;
}

impl StreamableUpdate for QuoteUpdate {
    fn stream_symbol(&self) -> &Symbol {
        match self.instrument.id() {
            IdentifierScheme::Security(sec) => &sec.symbol,
            IdentifierScheme::Prediction(_) => {
                // QuoteUpdate should always reference a security instrument in current routing.
                // If this ever changes, routing and gating must be updated accordingly.
                unreachable!("QuoteUpdate.instrument is not a security")
            }
        }
    }
    fn stream_ts(&self) -> DateTime<Utc> {
        self.ts
    }
}

impl StreamableUpdate for OptionUpdate {
    fn stream_symbol(&self) -> &Symbol {
        match self.instrument.id() {
            borsa_core::IdentifierScheme::Security(sec) => &sec.symbol,
            borsa_core::IdentifierScheme::Prediction(_) => {
                unreachable!("OptionUpdate.instrument is not a security")
            }
        }
    }
    fn stream_ts(&self) -> DateTime<Utc> {
        self.ts
    }
}

impl StreamableUpdate for CandleUpdate {
    fn stream_symbol(&self) -> &Symbol {
        match self.instrument.id() {
            IdentifierScheme::Security(sec) => &sec.symbol,
            IdentifierScheme::Prediction(_) => {
                unreachable!("CandleUpdate.instrument is not a security")
            }
        }
    }
    fn stream_ts(&self) -> DateTime<Utc> {
        self.candle.ts
    }
}

/// Adapter trait to start a stream for a given update type.
#[async_trait::async_trait]
pub trait StreamUpdateKind: StreamableUpdate {
    type Context: Send + Sync + Clone + 'static;
    /// Whether the provider can stream this update kind.
    fn can_stream(provider: &dyn BorsaConnector, ctx: &Self::Context) -> bool;
    /// Start a streaming session for this update type.
    async fn start_stream(
        provider: &dyn BorsaConnector,
        instruments: &[Instrument],
        ctx: &Self::Context,
    ) -> Result<(StreamHandle, mpsc::Receiver<Self>), BorsaError>
    where
        Self: Sized;
}

#[async_trait::async_trait]
impl StreamUpdateKind for QuoteUpdate {
    type Context = ();

    fn can_stream(provider: &dyn BorsaConnector, _ctx: &Self::Context) -> bool {
        provider.as_stream_provider().is_some()
    }
    async fn start_stream(
        provider: &dyn BorsaConnector,
        instruments: &[Instrument],
        _ctx: &Self::Context,
    ) -> Result<(StreamHandle, mpsc::Receiver<Self>), BorsaError> {
        match provider.as_stream_provider() {
            Some(sp) => sp.stream_quotes(instruments).await,
            None => Err(borsa_core::BorsaError::unsupported("stream_quotes")),
        }
    }
}

#[async_trait::async_trait]
impl StreamUpdateKind for OptionUpdate {
    type Context = ();

    fn can_stream(provider: &dyn BorsaConnector, _ctx: &Self::Context) -> bool {
        provider.as_option_stream_provider().is_some()
    }
    async fn start_stream(
        provider: &dyn BorsaConnector,
        instruments: &[Instrument],
        _ctx: &Self::Context,
    ) -> Result<(StreamHandle, mpsc::Receiver<Self>), BorsaError> {
        match provider.as_option_stream_provider() {
            Some(sp) => sp.stream_options(instruments).await,
            None => Err(borsa_core::BorsaError::unsupported("stream_options")),
        }
    }
}

#[async_trait::async_trait]
impl StreamUpdateKind for CandleUpdate {
    type Context = Interval;

    fn can_stream(provider: &dyn BorsaConnector, _ctx: &Self::Context) -> bool {
        provider.as_candle_stream_provider().is_some()
    }
    async fn start_stream(
        provider: &dyn BorsaConnector,
        instruments: &[Instrument],
        ctx: &Self::Context,
    ) -> Result<(StreamHandle, mpsc::Receiver<Self>), BorsaError> {
        match provider.as_candle_stream_provider() {
            Some(sp) => sp.stream_candles(instruments, *ctx).await,
            None => Err(borsa_core::BorsaError::unsupported("stream_candles")),
        }
    }
}
