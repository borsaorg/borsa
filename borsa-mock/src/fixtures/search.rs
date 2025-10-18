use borsa_core::{AssetKind, Exchange, SearchRequest, SearchResponse, SearchResult, Symbol};

pub fn search(req: &SearchRequest) -> SearchResponse {
    let q = req.query().to_ascii_lowercase();
    let mut results = Vec::new();
    if q.contains("tesla") {
        results.push(SearchResult {
            symbol: Symbol::new("TSLA").unwrap(),
            name: Some("Tesla Inc".to_string()),
            exchange: Some(Exchange::NASDAQ),
            kind: AssetKind::Equity,
        });
    }
    SearchResponse { results }
}
