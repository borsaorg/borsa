#![cfg(feature = "test-adapters")]

use borsa_core::{AssetKind, BorsaError, Instrument, connector::QuoteProvider};
use borsa_yfinance::{YfConnector, adapter};

use std::sync::Arc;

struct Combo {
    q: Arc<dyn adapter::YfQuotes>,
}
impl adapter::CloneArcAdapters for Combo {
    fn clone_arc_quotes(&self) -> Arc<dyn adapter::YfQuotes> {
        self.q.clone()
    }
}

#[tokio::test]
async fn empty_quotes_becomes_not_found() {
    // Quotes adapter returns empty -> NotFound
    let quotes = <dyn adapter::YfQuotes>::from_fn(|_symbols| Ok(vec![]));

    let yf = YfConnector::from_adapter(&Combo { q: quotes });

    let inst = Instrument::from_symbol("ABCD", AssetKind::Equity).expect("valid test instrument");
    let err = yf.quote(&inst).await.expect_err("should be not found");
    assert!(matches!(err, BorsaError::NotFound { .. }));
}
