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
    BorsaConnector, BorsaError, IdentifierScheme, Instrument, OptionUpdate, QuoteUpdate, Symbol,
    stream::StreamHandle,
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

/// Adapter trait to start a stream for a given update type.
#[async_trait::async_trait]
pub trait StreamUpdateKind: StreamableUpdate {
    /// Whether the provider can stream this update kind.
    fn can_stream(provider: &dyn BorsaConnector) -> bool;
    /// Start a streaming session for this update type.
    async fn start_stream(
        provider: &dyn BorsaConnector,
        instruments: &[Instrument],
    ) -> Result<(StreamHandle, mpsc::Receiver<Self>), BorsaError>
    where
        Self: Sized;
}

#[async_trait::async_trait]
impl StreamUpdateKind for QuoteUpdate {
    fn can_stream(provider: &dyn BorsaConnector) -> bool {
        provider.as_stream_provider().is_some()
    }
    async fn start_stream(
        provider: &dyn BorsaConnector,
        instruments: &[Instrument],
    ) -> Result<(StreamHandle, mpsc::Receiver<Self>), BorsaError> {
        match provider.as_stream_provider() {
            Some(sp) => sp.stream_quotes(instruments).await,
            None => Err(borsa_core::BorsaError::unsupported("stream_quotes")),
        }
    }
}

#[async_trait::async_trait]
impl StreamUpdateKind for OptionUpdate {
    fn can_stream(provider: &dyn BorsaConnector) -> bool {
        provider.as_option_stream_provider().is_some()
    }
    async fn start_stream(
        provider: &dyn BorsaConnector,
        instruments: &[Instrument],
    ) -> Result<(StreamHandle, mpsc::Receiver<Self>), BorsaError> {
        match provider.as_option_stream_provider() {
            Some(sp) => sp.stream_options(instruments).await,
            None => Err(borsa_core::BorsaError::unsupported("stream_options")),
        }
    }
}
