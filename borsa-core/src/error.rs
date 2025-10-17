use thiserror::Error;

/// Unified error type for the borsa workspace.
///
/// This wraps capability mismatches, argument validation errors, provider-tagged
/// failures, not-found conditions, and an aggregate for multi-provider attempts.
#[derive(Debug, Error)]
pub enum BorsaError {
    /// The requested capability is not implemented by the target connector.
    #[error("unsupported capability: {capability}")]
    Unsupported {
        /// A capability string describing what was requested (e.g. "history/crypto").
        capability: &'static str,
    },

    /// Issues with the returned or expected data (missing fields, etc.).
    #[error("data issue: {0}")]
    Data(String),

    /// Invalid input argument.
    #[error("invalid argument: {0}")]
    InvalidArg(String),

    /// An individual connector returned an error.
    #[error("{connector} failed: {msg}")]
    Connector {
        /// Connector name that failed.
        connector: String,
        /// Human-readable error message.
        msg: String,
    },

    /// Unknown/opaque error.
    #[error("unknown error: {0}")]
    Other(String),

    /// A resource or symbol could not be found.
    #[error("not found: {what}")]
    NotFound {
        /// Description of missing resource, e.g. "quote for AAPL".
        what: String,
    },

    /// All selected providers failed; contains the individual failures.
    #[error("all providers failed: {0:?}")]
    AllProvidersFailed(Vec<BorsaError>),

    /// An individual provider call exceeded the configured timeout.
    #[error("provider timed out: {capability} via {connector}")]
    ProviderTimeout {
        /// Connector name that timed out.
        connector: String,
        /// Capability label (e.g. "history", "search", "quote").
        capability: &'static str,
    },

    /// The overall request exceeded the configured deadline.
    #[error("request timed out: {capability}")]
    RequestTimeout {
        /// Capability label for which the request timed out.
        capability: &'static str,
    },

    /// All attempted providers timed out for the requested capability.
    #[error("all providers timed out: {capability}")]
    AllProvidersTimedOut {
        /// Capability label that timed out across all providers.
        capability: &'static str,
    },
}

impl BorsaError {
    /// Helper: build an `Unsupported` error for a capability string.
    #[must_use]
    pub const fn unsupported(cap: &'static str) -> Self {
        Self::Unsupported { capability: cap }
    }
    /// Helper: build a `Connector` error with the connector name and message.
    pub fn connector(connector: impl Into<String>, msg: impl Into<String>) -> Self {
        Self::Connector {
            connector: connector.into(),
            msg: msg.into(),
        }
    }

    /// Helper: build a `NotFound` error for a description of the missing resource.
    pub fn not_found(what: impl Into<String>) -> Self {
        Self::NotFound { what: what.into() }
    }

    /// Helper: build a `ProviderTimeout` error.
    pub fn provider_timeout(connector: impl Into<String>, capability: &'static str) -> Self {
        Self::ProviderTimeout {
            connector: connector.into(),
            capability,
        }
    }

    /// Helper: build a `RequestTimeout` error.
    #[must_use]
    pub const fn request_timeout(capability: &'static str) -> Self {
        Self::RequestTimeout { capability }
    }
}

impl From<paft::Error> for BorsaError {
    fn from(err: paft::Error) -> Self {
        use paft::Error as E;
        match err {
            // Money runtime issues indicate a data/operation problem at runtime
            E::Money(_) => Self::Data(err.to_string()),
            // Input/validation problems from parsers, request builders, or canonicalization
            E::Core(_) | E::Domain(_) | E::Market(_) | E::MoneyParse(_) | E::Canonical(_) => {
                Self::InvalidArg(err.to_string())
            }
        }
    }
}

impl From<paft::market::MarketError> for BorsaError {
    fn from(e: paft::market::MarketError) -> Self {
        Self::InvalidArg(e.to_string())
    }
}

impl From<paft::domain::DomainError> for BorsaError {
    fn from(e: paft::domain::DomainError) -> Self {
        Self::InvalidArg(e.to_string())
    }
}

impl From<paft::core::PaftError> for BorsaError {
    fn from(e: paft::core::PaftError) -> Self {
        Self::InvalidArg(e.to_string())
    }
}

impl From<paft::money::MoneyError> for BorsaError {
    fn from(e: paft::money::MoneyError) -> Self {
        Self::Data(e.to_string())
    }
}
