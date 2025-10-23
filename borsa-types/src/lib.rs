//! Borsa-specific data transfer objects and configuration primitives built on top of `paft`.
#![warn(missing_docs)]

mod attribution;
mod capability;
mod config;
mod connector;
mod error;
mod reports;
pub mod routing_policy;

pub use attribution::{Attribution, Span};
pub use capability::Capability;
pub use config::{BackoffConfig, BorsaConfig, FetchStrategy, MergeStrategy, Resampling};
pub use connector::ConnectorKey;
pub use error::BorsaError;
pub use reports::{DownloadReport, InfoReport, SearchReport};
pub use routing_policy::{
    Preference, RoutingContext, RoutingPolicy, RoutingPolicyBuilder, ScopeKey,
};
