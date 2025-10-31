use borsa::Borsa;
use borsa_core::{AssetKind, SearchRequest, SearchResult, Symbol};

use crate::helpers::m_search;

#[tokio::test]
#[allow(clippy::similar_names)]
async fn search_applies_limit_after_merge() {
    let aaa = Symbol::new("AAA").expect("valid symbol");
    let aab = Symbol::new("AAB").expect("valid symbol");
    let aac = Symbol::new("AAC").expect("valid symbol");
    let a = m_search(
        "a",
        vec![
            SearchResult {
                symbol: aaa.clone(),
                name: None,
                exchange: None,
                kind: AssetKind::Equity,
            },
            SearchResult {
                symbol: aab.clone(),
                name: None,
                exchange: None,
                kind: AssetKind::Equity,
            },
        ],
    );
    let b = m_search(
        "b",
        vec![SearchResult {
            symbol: aac.clone(),
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
