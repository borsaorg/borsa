//! Report envelopes produced by orchestrators and helpers.

use paft::aggregates::Info;
use paft::domain::Symbol;
use paft::market::responses::download::DownloadResponse;
use paft::market::responses::search::SearchResponse;
use serde::{Deserialize, Serialize};

use crate::error::BorsaError;

/// Summary of instrument information retrieval.
///
/// Carries the requested `symbol`, the resolved [`Info`] snapshot if
/// available, and any non-fatal warnings encountered during processing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct InfoReport {
    /// Requested symbol.
    pub symbol: Symbol,
    /// Snapshot payload, if successfully resolved.
    pub info: Option<Info>,
    /// Non-fatal issues encountered while building the report.
    pub warnings: Vec<BorsaError>,
}

/// Summary of a symbol search operation.
///
/// Contains the upstream search `response` when present and any associated
/// `warnings`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SearchReport {
    /// Upstream search response payload.
    pub response: Option<SearchResponse>,
    /// Non-fatal issues encountered while building the report.
    pub warnings: Vec<BorsaError>,
}

/// Summary of historical data download.
///
/// Wraps a [`DownloadResponse`] payload when present and any `warnings`
/// captured during retrieval or normalization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DownloadReport {
    /// Aggregated download payload.
    pub response: Option<DownloadResponse>,
    /// Non-fatal issues encountered while building the report.
    pub warnings: Vec<BorsaError>,
}
