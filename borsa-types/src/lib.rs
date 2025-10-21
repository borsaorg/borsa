//! Borsa-specific data transfer objects and configuration primitives built on top of `paft`.
#![warn(missing_docs)]

mod attribution;
mod config;
mod connector;
mod reports;

pub use attribution::{Attribution, Span};
pub use config::{BackoffConfig, BorsaConfig, FetchStrategy, MergeStrategy, Resampling};
pub use connector::ConnectorKey;
pub use reports::{DownloadReport, InfoReport, SearchReport};
