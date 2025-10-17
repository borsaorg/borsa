//! Time-series utilities shared by connectors and orchestrator.
//!
//! Modules include:
//! - `infer`: infer interval and detect gaps/continuity
//! - `merge`: merge multiple provider series respecting priority and adjusted preference
//! - `resample`: resample candles to requested cadence
/// Interval inference and sub-daily detection helpers.
pub mod infer;
/// Merge utilities for joining multiple history series.
pub mod merge;
/// Resampling utilities for aggregating candles to daily/weekly/minutes.
pub mod resample;
