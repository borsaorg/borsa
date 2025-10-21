//! Attribution types for merged history spans.

/// A continuous span of timestamps [start..=end] that a connector contributed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Inclusive start timestamp (seconds since epoch).
    pub start: i64,
    /// Inclusive end timestamp (seconds since epoch).
    pub end: i64,
}

/// Attribution of merged history: which connector supplied which timestamp spans.
///
/// Behavior:
/// - Built during history aggregation by tracking de-duplicated candle timestamps
///   and emitting spans whenever a provider contributes a contiguous range.
/// - Useful for debugging merge decisions and provider coverage over time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribution {
    /// Symbol the attribution refers to.
    pub symbol: String,
    /// Collected spans annotated by connector key.
    pub spans: Vec<(&'static str, Span)>, // (connector_name, span)
}

impl Attribution {
    /// Create a new attribution container for a symbol.
    #[must_use]
    pub const fn new(symbol: String) -> Self {
        Self {
            symbol,
            spans: vec![],
        }
    }

    /// Record a provider span contribution.
    pub fn push(&mut self, item: (&'static str, Span)) {
        self.spans.push(item);
    }
}
