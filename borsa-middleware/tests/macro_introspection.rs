use std::sync::Arc;

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
#[borsa_macros::delegate_all_providers(inner, pre_call = "let _ = ();")]
impl MacroIntrospectWrapper {
    fn new(inner: Arc<dyn BorsaConnector>) -> Self {
        Self { inner }
    }
}

#[test]
fn macro_introspection_compiles_and_delegates_name() {
    let raw: Arc<dyn BorsaConnector> = Arc::new(NullConnector);
    let wrapped = MacroIntrospectWrapper::new(raw);
    assert_eq!(wrapped.name(), "null");
}
