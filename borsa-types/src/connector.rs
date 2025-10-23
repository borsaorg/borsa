//! Connector metadata types usable across crates.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

#[derive(Debug, Clone)]
enum ConnectorKeyInner {
    Static(&'static str),
    Shared(Arc<str>),
}

/// Typed key for identifying connectors in priority configuration.
#[derive(Debug, Clone)]
pub struct ConnectorKey(ConnectorKeyInner);

impl ConnectorKey {
    /// Construct a connector key from a `'static` string without allocating.
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        Self(ConnectorKeyInner::Static(name))
    }

    /// Construct a connector key by taking ownership of the provided string.
    #[must_use]
    pub fn from_owned<S: Into<Arc<str>>>(name: S) -> Self {
        Self(ConnectorKeyInner::Shared(name.into()))
    }

    /// Returns the inner string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match &self.0 {
            ConnectorKeyInner::Static(s) => s,
            ConnectorKeyInner::Shared(s) => s.as_ref(),
        }
    }
}

impl Hash for ConnectorKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl PartialEq for ConnectorKey {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for ConnectorKey {}

impl Serialize for ConnectorKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ConnectorKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_owned(s))
    }
}

impl From<&'static str> for ConnectorKey {
    fn from(s: &'static str) -> Self {
        Self::new(s)
    }
}

impl From<String> for ConnectorKey {
    fn from(s: String) -> Self {
        Self::from_owned(Arc::<str>::from(s))
    }
}

impl fmt::Display for ConnectorKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
