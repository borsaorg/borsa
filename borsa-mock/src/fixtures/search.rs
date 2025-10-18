use borsa_core::{SearchRequest, SearchResponse, BorsaError, SearchResult, Symbol, Exchange, AssetKind};

pub fn search(req: &SearchRequest) -> Result<SearchResponse, BorsaError> {
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
    Ok(SearchResponse { results })
}
