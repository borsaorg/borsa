use borsa::Borsa;
use borsa_core::{AssetKind, Exchange, SearchRequest, SearchResult, Symbol};

use crate::helpers::m_search;

#[tokio::test]
async fn search_respects_per_kind_priority_and_dedups() {
    // Both connectors return AAPL, but with different names.
    let low = m_search(
        "low",
        vec![
            SearchResult {
                symbol: Symbol::new("AAPL").unwrap(),
                name: Some("Apple Inc. LOW".into()),
                exchange: Exchange::try_from_str("NasdaqGS").ok(),
                kind: AssetKind::Equity,
            },
            SearchResult {
                symbol: Symbol::new("AA").unwrap(),
                name: Some("Alcoa".into()),
                exchange: Exchange::try_from_str("NYSE").ok(),
                kind: AssetKind::Equity,
            },
        ],
    );

    let high = m_search(
        "high",
        vec![
            SearchResult {
                symbol: Symbol::new("AAPL").unwrap(),
                name: Some("Apple Inc. HIGH".into()),
                exchange: Exchange::try_from_str("NasdaqGS").ok(),
                kind: AssetKind::Equity,
            },
            SearchResult {
                symbol: Symbol::new("MSFT").unwrap(),
                name: Some("Microsoft".into()),
                exchange: Exchange::try_from_str("NasdaqGS").ok(),
                kind: AssetKind::Equity,
            },
        ],
    );

    let borsa = Borsa::builder()
        .with_connector(low.clone())
        .with_connector(high.clone())
        .prefer_for_kind(AssetKind::Equity, &[high, low])
        .build()
        .unwrap();

    let req = SearchRequest::builder("apple")
        .kind(AssetKind::Equity)
        .build()
        .unwrap();
    let out = borsa.search(req).await.unwrap();

    // Expect: AAPL from "high" wins; AA from low, MSFT from high (order by provider priority preserved).
    let syms: Vec<_> = out
        .response
        .as_ref()
        .unwrap()
        .results
        .iter()
        .map(|r| r.symbol.as_str())
        .collect();
    assert_eq!(syms, vec!["AAPL", "MSFT", "AA"]);
    let aapl = &out.response.as_ref().unwrap().results[0];
    assert_eq!(aapl.name.as_deref(), Some("Apple Inc. HIGH"));
}
