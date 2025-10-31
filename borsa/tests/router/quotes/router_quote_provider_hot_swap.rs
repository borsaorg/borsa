use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use async_trait::async_trait;
use borsa::Borsa;
use borsa_core::{AssetKind, BorsaConnector, BorsaError, Instrument, Quote};

use crate::helpers::{MockConnector, X};

struct HotSwapQuoteConnector {
    inner: Arc<MockConnector>,
    arm_drop: AtomicBool,
    seen_precheck: AtomicBool,
}

#[tokio::test]
async fn quote_capability_hot_swap_returns_error_not_panic() {
    // Build a mock that initially advertises quote capability, but we will
    // remove it after the macro's pre-check to simulate a hot-swap.
    let base = crate::helpers::MockConnector::builder()
        .name("hot")
        .returns_quote_ok(crate::helpers::quote_fixture(&X, "1.00"))
        .build();

    let hs = Arc::new(HotSwapQuoteConnector {
        inner: base.clone(),
        arm_drop: AtomicBool::new(false),
        seen_precheck: AtomicBool::new(false),
    });

    // Wrap in an Arc<dyn BorsaConnector>
    let hs_arc: Arc<dyn BorsaConnector> = hs.clone();

    // Prepare orchestrator
    let borsa = Borsa::builder().with_connector(hs_arc).build().unwrap();

    // First call: ensure success to confirm capability present
    let inst = crate::helpers::instrument(&X, AssetKind::Equity);
    let ok = borsa.quote(&inst).await;
    assert!(ok.is_ok());

    // Flip the switch: the provider will disappear during the call
    hs.arm_drop.store(true, Ordering::SeqCst);

    // Second call should not panic; it should return a connector error
    let res = borsa.quote(&inst).await;
    match res {
        Err(BorsaError::AllProvidersFailed(v)) => {
            assert_eq!(v.len(), 1);
            match &v[0] {
                BorsaError::Connector { connector, msg } => {
                    assert!(connector.contains("hot"));
                    assert!(msg.contains("missing quote capability during call"));
                }
                other => panic!("unexpected error variant: {other:?}"),
            }
        }
        other => panic!("expected AllProvidersFailed, got {other:?}"),
    }
}

#[async_trait]
impl borsa_core::connector::QuoteProvider for HotSwapQuoteConnector {
    async fn quote(&self, instrument: &Instrument) -> Result<Quote, BorsaError> {
        borsa_core::connector::QuoteProvider::quote(&*self.inner, instrument).await
    }
}

impl BorsaConnector for HotSwapQuoteConnector {
    fn name(&self) -> &'static str {
        "hot"
    }

    fn supports_kind(&self, kind: AssetKind) -> bool {
        self.inner.supports_kind(kind)
    }

    fn as_quote_provider(&self) -> Option<&dyn borsa_core::connector::QuoteProvider> {
        // If armed, allow the first pre-check to pass, then drop for the next check
        if self.arm_drop.load(Ordering::SeqCst) {
            if self.seen_precheck.swap(true, Ordering::SeqCst) {
                self.arm_drop.store(false, Ordering::SeqCst);
                self.seen_precheck.store(false, Ordering::SeqCst);
                None
            } else {
                Some(self as &dyn borsa_core::connector::QuoteProvider)
            }
        } else {
            Some(self as &dyn borsa_core::connector::QuoteProvider)
        }
    }
}
