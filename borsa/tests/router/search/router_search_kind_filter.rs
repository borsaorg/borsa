use borsa::Borsa;

use borsa_core::{AssetKind, Exchange, SearchRequest, SearchResult, Symbol};

use crate::helpers::mock_connector::MockConnector;

#[tokio::test]
async fn search_respects_connector_kind_support() {
    // Equity-only connector returns SPY (fund) but claims it supports only Equity -> should be filtered out when kind=Fund.
    let spy = Symbol::new("SPY").expect("valid symbol");
    let equity_only = MockConnector::builder()
        .name("eq")
        .supports_kind(AssetKind::Equity)
        .returns_search_ok(vec![SearchResult {
            symbol: spy.clone(),
            name: Some("SPDR S&P 500 ETF".into()),
            exchange: Exchange::try_from_str("NYSEArca").ok(),
            kind: AssetKind::Fund,
        }])
        .build();

    // Fund-capable connector returns SPY
    let fund_capable = MockConnector::builder()
        .name("fund")
        .supports_kind(AssetKind::Fund)
        .returns_search_ok(vec![SearchResult {
            symbol: spy.clone(),
            name: Some("SPDR S&P 500 ETF".into()),
            exchange: Exchange::try_from_str("NYSEArca").ok(),
            kind: AssetKind::Fund,
        }])
        .build();

    let borsa = Borsa::builder()
        .with_connector(equity_only)
        .with_connector(fund_capable)
        .build()
        .unwrap();

    let req = SearchRequest::builder("spy")
        .kind(AssetKind::Fund)
        .build()
        .unwrap();
    let out = borsa.search(req).await.unwrap();

    let resp = out.response.unwrap();
    let syms: Vec<_> = resp.results.iter().map(|r| r.symbol.as_str()).collect();
    assert_eq!(
        syms,
        vec!["SPY"],
        "should have used only the fund-capable connector"
    );
}
