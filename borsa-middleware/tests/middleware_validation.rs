use std::any::TypeId;
use std::sync::Arc;

use borsa_core::{
    BorsaError, Middleware, connector::BorsaConnector, middleware::ValidationContext,
};
use borsa_middleware::ConnectorBuilder;
use borsa_mock::MockConnector;

/// Example custom middleware that requires another specific middleware to be present.
struct CustomMiddleware;

impl Middleware for CustomMiddleware {
    fn apply(self: Box<Self>, inner: Arc<dyn BorsaConnector>) -> Arc<dyn BorsaConnector> {
        inner
    }

    fn name(&self) -> &'static str {
        "CustomMiddleware"
    }

    fn config_json(&self) -> serde_json::Value {
        serde_json::json!({})
    }

    fn validate(&self, ctx: &ValidationContext) -> Result<(), BorsaError> {
        // Require that QuotaAware middleware is present somewhere in the stack
        if !ctx.has_middleware(TypeId::of::<borsa_middleware::QuotaMiddleware>()) {
            return Err(BorsaError::InvalidMiddlewareStack {
                message: "CustomMiddleware requires QuotaMiddleware to be present".to_string(),
            });
        }
        Ok(())
    }
}

#[test]
fn validation_fails_when_dependency_missing() {
    let raw: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());
    let result = ConnectorBuilder::new(raw).layer(CustomMiddleware).build();

    assert!(result.is_err());
    match result {
        Err(BorsaError::InvalidMiddlewareStack { message }) => {
            assert!(message.contains("QuotaMiddleware"));
        }
        _ => panic!("Expected InvalidMiddlewareStack error"),
    }
}

#[test]
fn validation_succeeds_when_dependency_present() {
    let raw: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());
    let cfg = borsa_types::QuotaConfig::default();
    let result = ConnectorBuilder::new(raw)
        .with_quota(&cfg)
        .layer(CustomMiddleware)
        .build();

    assert!(result.is_ok());
    let connector = result.unwrap();
    assert_eq!(connector.name(), "borsa-mock");
}

#[test]
fn validation_context_provides_correct_position_info() {
    struct PositionCheckingMiddleware;

    impl Middleware for PositionCheckingMiddleware {
        fn apply(self: Box<Self>, inner: Arc<dyn BorsaConnector>) -> Arc<dyn BorsaConnector> {
            inner
        }

        fn name(&self) -> &'static str {
            "PositionCheckingMiddleware"
        }

        fn config_json(&self) -> serde_json::Value {
            serde_json::json!({})
        }

        fn validate(&self, ctx: &ValidationContext) -> Result<(), BorsaError> {
            // Check that we can query middleware positions correctly
            let has_quota_outer =
                ctx.has_middleware_outer(TypeId::of::<borsa_middleware::QuotaMiddleware>());
            let has_blacklist_outer =
                ctx.has_middleware_outer(TypeId::of::<borsa_middleware::BlacklistMiddleware>());

            // This middleware should be innermost, so nothing should be inner
            let has_quota_inner =
                ctx.has_middleware_inner(TypeId::of::<borsa_middleware::QuotaMiddleware>());

            if !has_quota_outer || !has_blacklist_outer {
                return Err(BorsaError::InvalidMiddlewareStack {
                    message: "Expected Quota and Blacklist to be outer".to_string(),
                });
            }

            if has_quota_inner {
                return Err(BorsaError::InvalidMiddlewareStack {
                    message: "Expected to be innermost".to_string(),
                });
            }

            Ok(())
        }
    }

    let raw: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());
    let cfg = borsa_types::QuotaConfig::default();
    let result = ConnectorBuilder::new(raw)
        .layer(PositionCheckingMiddleware) // This will be innermost
        .with_quota(&cfg) // This will be middle
        .with_blacklist(std::time::Duration::from_secs(60)) // This will be outermost
        .build();

    assert!(result.is_ok());
}
