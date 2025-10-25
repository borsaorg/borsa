//! Middleware trait for wrapping `BorsaConnector` implementations.

use std::sync::Arc;

use crate::connector::BorsaConnector;

/// Trait implemented by connector middleware layers.
///
/// A middleware consumes an inner `BorsaConnector` and returns a wrapped connector
/// that augments or restricts behavior (e.g., quotas, blacklisting).
pub trait Middleware: Send + Sync {
    /// Apply this middleware to wrap an inner connector and return the wrapped connector.
    fn apply(self: Box<Self>, inner: Arc<dyn BorsaConnector>) -> Arc<dyn BorsaConnector>;

    /// Human-readable middleware name for introspection/logging.
    fn name(&self) -> &'static str;

    /// Opaque configuration snapshot for serialization/inspection.
    fn config_json(&self) -> serde_json::Value;
}
