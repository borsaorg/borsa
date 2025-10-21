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
/// Internal stream utilities used by `StreamHandle` and tests.
pub mod stream;
/// Time-series utilities for merging and resampling.
pub mod timeseries;
pub mod types;

pub use connector::BorsaConnector;
pub use timeseries::infer::{estimate_step_seconds, is_subdaily};
pub use timeseries::merge::{dedup_actions, merge_candles_by_priority, merge_history};
pub use timeseries::resample::{resample_to_daily, resample_to_minutes, resample_to_weekly};
pub use types::*;
