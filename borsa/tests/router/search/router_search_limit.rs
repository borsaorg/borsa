use borsa::Borsa;
use borsa_core::{AssetKind, SearchRequest, SearchResult, Symbol};

use crate::helpers::m_search;

#[tokio::test]
async fn search_applies_limit_after_merge() {
    let a = m_search(
        "a",
        vec![
            SearchResult {
                symbol: Symbol::new("AAA").unwrap(),
                name: None,
                exchange: None,
                kind: AssetKind::Equity,
            },
            SearchResult {
                symbol: Symbol::new("AAB").unwrap(),
                name: None,
                exchange: None,
                kind: AssetKind::Equity,
            },
        ],
    );
    let b = m_search(
        "b",
        vec![SearchResult {
            symbol: Symbol::new("AAC").unwrap(),
            name: None,
            exchange: None,
            kind: AssetKind::Equity,
        }],
    );

    let borsa = Borsa::builder()
        .with_connector(a)
        .with_connector(b)
        .build()
        .unwrap();

    let req = SearchRequest::builder("aa")
        .kind(AssetKind::Equity)
        .limit(2)
        .build()
        .unwrap();
    let out = borsa.search(req).await.unwrap();

    let resp = out.response.unwrap();
    let syms: Vec<_> = resp.results.iter().map(|r| r.symbol.as_str()).collect();
    assert_eq!(syms, vec!["AAA", "AAB"]);
}
