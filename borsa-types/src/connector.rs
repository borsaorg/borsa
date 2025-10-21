//! Connector metadata types usable across crates.

/// Typed key for identifying connectors in priority configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectorKey(pub &'static str);

impl ConnectorKey {
    /// Construct a new typed connector key from a static name.
    ///
    /// This is useful when configuring per-kind or per-symbol priorities.
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    /// Returns the inner static string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl From<ConnectorKey> for &'static str {
    fn from(k: ConnectorKey) -> Self {
        k.0
    }
}
