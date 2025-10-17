#![cfg(feature = "test-adapters")]

use std::sync::Arc;

use borsa_core::{SearchRequest, connector::SearchProvider};
use borsa_yfinance::{YfConnector, adapter};

struct Combo {
    s: Arc<dyn adapter::YfSearch>,
}

impl adapter::CloneArcAdapters for Combo {
    fn clone_arc_search(&self) -> Arc<dyn adapter::YfSearch> {
        self.s.clone()
    }
}

#[tokio::test]
async fn search_uses_injected_adapter_symbol_only() {
    // Fake search adapter returns a few symbols for the query.
    let search = <dyn adapter::YfSearch>::from_fn(|q| {
        assert_eq!(q, "apple");
        Ok(vec![
            "AAPL".to_string(),
            "AAP".to_string(),
            "APLE".to_string(),
        ])
    });

    let yf = YfConnector::from_adapter(&Combo { s: search });

    let req = SearchRequest::new("apple").unwrap();
    let resp = yf.search(req).await.unwrap();

    let syms: Vec<_> = resp.results.iter().map(|r| r.symbol.as_str()).collect();
    assert_eq!(syms, vec!["AAPL", "AAP", "APLE"]);
}
