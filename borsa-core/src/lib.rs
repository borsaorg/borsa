//! borsa-core
//!
//! Core types, traits, and utilities shared across the borsa ecosystem.
//!
//! - `types`: common data structures (quotes, candles, actions, requests).
//! - `connector`: the `BorsaConnector` trait and capability provider traits.
//! - `timeseries`: helpers to merge history from multiple connectors.
//!
//! Async runtime (Tokio)
//! ---------------------
//! This crate assumes the Tokio ecosystem as the async runtime. Several public
//! APIs are explicitly coupled to Tokio types and facilities:
//!
//! - `stream::StreamHandle` wraps `tokio::task::JoinHandle<()>` and uses
//!   `tokio::sync::oneshot::Sender<()>` for cooperative shutdown.
//! - `connector::StreamProvider` returns `(StreamHandle, tokio::sync::mpsc::Receiver<QuoteUpdate>)`.
//! - `middleware::CallOrigin` uses `tokio::task_local!` to track call origin
//!   across async boundaries.
//!
//! As a result, code that uses streaming or middleware must run under a Tokio
//! 1.x runtime.
//!
#![warn(missing_docs)]

/// Connector capability traits and the primary `BorsaConnector` interface.
pub mod connector;
/// Middleware trait implemented by connector wrappers.
pub mod middleware;
/// Internal stream utilities used by `StreamHandle` and tests.
pub mod stream;
/// Time-series utilities for merging and resampling.
pub mod timeseries;
pub mod types;

pub use connector::BorsaConnector;
pub use middleware::{
    CallContext, CallOrigin, Middleware, MiddlewareDescriptor, MiddlewarePosition,
    ValidationContext,
};
pub use timeseries::infer::{estimate_step_seconds, is_subdaily};
pub use timeseries::merge::{dedup_actions, merge_candles_by_priority, merge_history};
pub use timeseries::resample::{resample_to_daily, resample_to_minutes, resample_to_weekly};
pub use types::*;
