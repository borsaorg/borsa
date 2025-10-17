//! borsa-core
//!
//! Core types, traits, and utilities shared across the borsa ecosystem.
//!
//! - `types`: common data structures (quotes, candles, actions, requests).
//! - `connector`: the `BorsaConnector` trait and capability provider traits.
//! - `timeseries`: helpers to merge history from multiple connectors.
#![warn(missing_docs)]

/// Connector capability traits and the primary `BorsaConnector` interface.
pub mod connector;
/// Core error type shared by orchestrator and connectors.
pub mod error;
/// Internal stream utilities used by `StreamHandle` and tests.
pub mod stream;
/// Time-series utilities for merging and resampling.
pub mod timeseries;
pub mod types;

/// Minimal stream handle abstraction for long-lived streaming tasks.
///
/// Lifecycle contract:
/// - Prefer calling [`stop`](StreamHandle::stop) to request a graceful shutdown and await completion.
/// - Call [`abort`](StreamHandle::abort) for immediate, non-graceful termination.
/// - If dropped without an explicit shutdown, a best-effort stop signal is sent (if available) and
///   the underlying task is then aborted. The task may not observe the stop signal before abort.
#[derive(Debug)]
pub struct StreamHandle {
    inner: Option<tokio::task::JoinHandle<()>>,
    stop_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl StreamHandle {
    /// Create a new `StreamHandle`.
    ///
    /// Parameters:
    /// - `inner`: the spawned task driving the stream.
    /// - `stop_tx`: a one-shot used to request a graceful stop.
    ///
    /// Returns a handle that can be used to stop or abort the stream.
    #[must_use]
    pub const fn new(
        inner: tokio::task::JoinHandle<()>,
        stop_tx: tokio::sync::oneshot::Sender<()>,
    ) -> Self {
        Self {
            inner: Some(inner),
            stop_tx: Some(stop_tx),
        }
    }

    /// Create a `StreamHandle` that can only abort the task (no graceful stop).
    ///
    /// This constructor is intended for connectors that do not support a
    /// cooperative shutdown signal. Dropping the handle (or calling
    /// [`abort`](Self::abort)) will force-cancel the underlying task.
    #[must_use]
    pub const fn new_abort_only(inner: tokio::task::JoinHandle<()>) -> Self {
        Self {
            inner: Some(inner),
            stop_tx: None,
        }
    }

    /// Gracefully stop the underlying stream task and await its completion.
    ///
    /// Sends a stop signal if available, then awaits the task. Any errors
    /// from the task are ignored.
    pub async fn stop(mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(inner) = self.inner.take() {
            let _ = inner.await;
        }
    }

    /// Force-abort the underlying stream task without waiting for completion.
    ///
    /// Prefer [`stop`](Self::stop) when possible to allow cleanup.
    pub fn abort(mut self) {
        if let Some(inner) = self.inner.take() {
            inner.abort();
        }
    }
}

impl Drop for StreamHandle {
    fn drop(&mut self) {
        crate::stream::drop_impl(&mut self.inner, &mut self.stop_tx);
    }
}

pub use connector::BorsaConnector;
pub use error::BorsaError;
pub use timeseries::infer::{estimate_step_seconds, is_subdaily};
pub use timeseries::merge::{dedup_actions, merge_candles_by_priority, merge_history};
pub use timeseries::resample::{resample_to_daily, resample_to_minutes, resample_to_weekly};
pub use types::*;
