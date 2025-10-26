//! Middleware trait for wrapping `BorsaConnector` implementations.

use std::any::{Any, TypeId};
use std::sync::Arc;

use crate::BorsaError;
use crate::connector::BorsaConnector;

/// Position requirement for middleware in the stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MiddlewarePosition {
    /// This middleware must be outermost (applied last, handles requests first).
    Outermost,
    /// This middleware must be outside (closer to user than) the specified middleware type.
    OuterThan(TypeId),
    /// This middleware must be inside (closer to raw connector than) the specified middleware type.
    InnerThan(TypeId),
    /// No position requirement.
    Any,
}

/// Validation context passed to middleware during stack validation.
///
/// Allows middleware to inspect the full stack and verify dependencies.
pub struct ValidationContext<'a> {
    /// All middleware in the stack, ordered from outermost to innermost.
    /// Index 0 is outermost (closest to user), last index is innermost (closest to raw connector).
    stack: &'a [MiddlewareDescriptor],
    /// Index of the middleware being validated in the stack.
    current_index: usize,
}

impl<'a> ValidationContext<'a> {
    /// Create a new validation context.
    ///
    /// # Arguments
    /// * `stack` - All middleware descriptors in the stack, ordered from outermost to innermost
    /// * `current_index` - Index of the middleware being validated
    #[must_use]
    pub const fn new(stack: &'a [MiddlewareDescriptor], current_index: usize) -> Self {
        Self {
            stack,
            current_index,
        }
    }

    /// Check if a middleware type exists in the stack.
    #[must_use] 
    pub fn has_middleware(&self, type_id: TypeId) -> bool {
        self.stack.iter().any(|m| m.type_id() == type_id)
    }

    /// Check if a middleware type exists outer than (closer to user than) the current middleware.
    ///
    /// Since the stack is stored outermost-first, "outer" means lower indices.
    #[must_use] 
    pub fn has_middleware_outer(&self, type_id: TypeId) -> bool {
        self.stack[..self.current_index]
            .iter()
            .any(|m| m.type_id() == type_id)
    }

    /// Check if a middleware type exists inner than (closer to connector than) the current middleware.
    ///
    /// Since the stack is stored outermost-first, "inner" means higher indices.
    #[must_use] 
    pub fn has_middleware_inner(&self, type_id: TypeId) -> bool {
        self.stack[self.current_index + 1..]
            .iter()
            .any(|m| m.type_id() == type_id)
    }

    /// Get all middleware type IDs in the stack, ordered outermost to innermost.
    #[must_use] 
    pub fn middleware_types(&self) -> Vec<TypeId> {
        self.stack.iter().map(MiddlewareDescriptor::type_id).collect()
    }

    /// Get the middleware's position in the stack (0 = outermost, n-1 = innermost).
    #[must_use] 
    pub const fn current_position(&self) -> usize {
        self.current_index
    }

    /// Get the total number of middleware in the stack.
    #[must_use] 
    pub const fn stack_size(&self) -> usize {
        self.stack.len()
    }
}

/// Internal descriptor for tracking middleware in the builder.
pub struct MiddlewareDescriptor {
    middleware: Box<dyn Middleware>,
    type_id: TypeId,
    name: &'static str,
}

impl MiddlewareDescriptor {
    /// Create a new middleware descriptor from a concrete middleware implementation.
    pub fn new<M: Middleware + 'static>(middleware: M) -> Self {
        let name = middleware.name();
        Self {
            middleware: Box::new(middleware),
            type_id: TypeId::of::<M>(),
            name,
        }
    }

    /// Get the type ID of the wrapped middleware.
    #[must_use] 
    pub const fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Get the human-readable name of the middleware.
    #[must_use] 
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Get a reference to the wrapped middleware trait object.
    #[must_use] 
    pub fn middleware(&self) -> &dyn Middleware {
        &*self.middleware
    }

    /// Consume this descriptor and extract the boxed middleware.
    #[must_use] 
    pub fn into_middleware(self) -> Box<dyn Middleware> {
        self.middleware
    }
}

/// Trait implemented by connector middleware layers.
///
/// A middleware consumes an inner `BorsaConnector` and returns a wrapped connector
/// that augments or restricts behavior (e.g., quotas, blacklisting).
///
/// Middleware can declare dependencies and position requirements to ensure correct
/// composition without hardcoding or footguns.
pub trait Middleware: Send + Sync {
    /// Apply this middleware to wrap an inner connector and return the wrapped connector.
    fn apply(self: Box<Self>, inner: Arc<dyn BorsaConnector>) -> Arc<dyn BorsaConnector>;

    /// Human-readable middleware name for introspection/logging.
    fn name(&self) -> &'static str;

    /// Opaque configuration snapshot for serialization/inspection.
    fn config_json(&self) -> serde_json::Value;

    /// Validate this middleware's position and dependencies in the stack.
    ///
    /// Called during builder validation before any middleware is applied.
    /// Allows middleware to enforce ordering requirements and dependencies.
    ///
    /// # Errors
    /// Return an error if validation fails (missing dependencies, wrong order, etc.).
    fn validate(&self, _ctx: &ValidationContext) -> Result<(), BorsaError> {
        Ok(())
    }

    /// Optional: Get this middleware as `&dyn Any` for downcasting.
    ///
    /// Default implementation returns None. Middleware can override to support
    /// runtime inspection without exposing concrete types.
    fn as_any(&self) -> Option<&dyn Any> {
        None
    }
}

/// Helper macro for middleware to check dependencies without hardcoding strings.
///
/// # Example
/// ```ignore
/// fn validate(&self, ctx: &ValidationContext) -> Result<(), BorsaError> {
///     require_middleware_outer!(ctx, BlacklistingMiddleware, "QuotaAware requires Blacklisting to be outermost");
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! require_middleware_outer {
    ($ctx:expr, $middleware_type:ty, $msg:expr) => {
        if !$ctx.has_middleware_outer(std::any::TypeId::of::<$middleware_type>()) {
            return Err($crate::BorsaError::InvalidMiddlewareStack {
                message: format!(
                    "{}: {} must be outside (outermost from) this middleware",
                    $msg,
                    std::any::type_name::<$middleware_type>()
                ),
            });
        }
    };
}

/// Helper macro for middleware to check that a dependency exists anywhere in the stack.
#[macro_export]
macro_rules! require_middleware {
    ($ctx:expr, $middleware_type:ty, $msg:expr) => {
        if !$ctx.has_middleware(std::any::TypeId::of::<$middleware_type>()) {
            return Err($crate::BorsaError::InvalidMiddlewareStack {
                message: format!(
                    "{}: {} must be present in the stack",
                    $msg,
                    std::any::type_name::<$middleware_type>()
                ),
            });
        }
    };
}

/// Helper macro to check middleware is inner than another.
#[macro_export]
macro_rules! require_middleware_inner {
    ($ctx:expr, $middleware_type:ty, $msg:expr) => {
        if !$ctx.has_middleware_inner(std::any::TypeId::of::<$middleware_type>()) {
            return Err($crate::BorsaError::InvalidMiddlewareStack {
                message: format!(
                    "{}: {} must be inside (innermost from) this middleware",
                    $msg,
                    std::any::type_name::<$middleware_type>()
                ),
            });
        }
    };
}
