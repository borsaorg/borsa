use serde::{Deserialize, Serialize};

/// A single middleware layer with a name and free-form JSON configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiddlewareLayer {
    /// Human-readable layer name (e.g., "`BlacklistingMiddleware`", "`QuotaAwareConnector`").
    pub name: String,
    /// Opaque configuration blob; concrete layers should document their schema.
    pub config: serde_json::Value,
}

impl MiddlewareLayer {
    /// Convenience constructor.
    #[must_use]
    pub fn new<N: Into<String>>(name: N, config: serde_json::Value) -> Self {
        Self {
            name: name.into(),
            config,
        }
    }
}

/// An ordered stack of middleware layers representing the onion of wrappers.
///
/// Convention: `layers[0]` is the OUTERMOST layer, the last element is the
/// INNERMOST layer (typically the raw connector).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiddlewareStack {
    /// Ordered list of layers, outermost first.
    pub layers: Vec<MiddlewareLayer>,
}

impl Default for MiddlewareStack {
    fn default() -> Self {
        Self::new()
    }
}

impl MiddlewareStack {
    /// Create an empty stack.
    #[must_use]
    pub const fn new() -> Self {
        Self { layers: Vec::new() }
    }

    /// Push a layer at the outermost position.
    pub fn push_outer(&mut self, layer: MiddlewareLayer) {
        self.layers.insert(0, layer);
    }

    /// Append a layer as the innermost one.
    pub fn push_inner(&mut self, layer: MiddlewareLayer) {
        self.layers.push(layer);
    }
}
