use std::sync::Arc;

use borsa_core::Middleware;
use borsa_core::connector::BorsaConnector;

struct NullConnector;

impl BorsaConnector for NullConnector {
    fn name(&self) -> &'static str {
        "null"
    }
}

struct MacroIntrospectWrapper {
    inner: Arc<dyn BorsaConnector>,
}

#[borsa_macros::delegate_connector(inner)]
#[borsa_macros::delegate_all_providers(inner)]
impl MacroIntrospectWrapper {
    fn new(inner: Arc<dyn BorsaConnector>) -> Self {
        Self { inner }
    }
}

#[async_trait::async_trait]
impl Middleware for MacroIntrospectWrapper {
    fn apply(self: Box<Self>, _inner: Arc<dyn BorsaConnector>) -> Arc<dyn BorsaConnector> {
        unreachable!()
    }
    fn name(&self) -> &'static str {
        "test"
    }
    fn config_json(&self) -> serde_json::Value {
        serde_json::json!({})
    }
}

#[test]
fn macro_introspection_compiles_and_delegates_name() {
    let raw: Arc<dyn BorsaConnector> = Arc::new(NullConnector);
    let wrapped = MacroIntrospectWrapper::new(raw);
    assert_eq!(
        borsa_core::connector::BorsaConnector::name(&wrapped),
        "null"
    );
}
