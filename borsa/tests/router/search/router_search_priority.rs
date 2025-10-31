use borsa::Borsa;
use borsa_core::{
    AssetKind, BorsaConnector, Exchange, RoutingPolicyBuilder, SearchRequest, SearchResult, Symbol,
};

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

    let policy = RoutingPolicyBuilder::new()
        .providers_for_kind(AssetKind::Equity, &[high.key(), low.key()])
        .build();
    let borsa = Borsa::builder()
        .with_connector(low.clone())
        .with_connector(high.clone())
        .routing_policy(policy)
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

#[tokio::test]
async fn search_respects_exchange_priority_kind_and_symbol_override() {
    // Both connectors return the same symbol with different exchanges and names.
    let low = m_search(
        "low",
        vec![
            SearchResult {
                symbol: Symbol::new("RIO").unwrap(),
                name: Some("Rio plc (LOW)".into()),
                exchange: Exchange::try_from_str("NYSE").ok(),
                kind: AssetKind::Equity,
            },
            SearchResult {
                symbol: Symbol::new("AAA").unwrap(),
                name: Some("AAA LOW".into()),
                exchange: Exchange::try_from_str("NYSE").ok(),
                kind: AssetKind::Equity,
            },
        ],
    );

    let high = m_search(
        "high",
        vec![
            SearchResult {
                symbol: Symbol::new("RIO").unwrap(),
                name: Some("Rio plc (HIGH)".into()),
                exchange: Exchange::try_from_str("LSE").ok(),
                kind: AssetKind::Equity,
            },
            SearchResult {
                symbol: Symbol::new("BBB").unwrap(),
                name: Some("BBB HIGH".into()),
                exchange: Exchange::try_from_str("NYSE").ok(),
                kind: AssetKind::Equity,
            },
        ],
    );

    let rio = Symbol::new("RIO").expect("valid symbol");
    let policy = RoutingPolicyBuilder::new()
        .exchanges_for_kind(
            AssetKind::Equity,
            &[
                Exchange::try_from_str("LSE").unwrap(),
                Exchange::try_from_str("NYSE").unwrap(),
            ],
        )
        .exchanges_for_symbol(&rio, &[Exchange::try_from_str("NYSE").unwrap()])
        .build();
    let borsa = Borsa::builder()
        .with_connector(low.clone())
        .with_connector(high.clone())
        .routing_policy(policy)
        .build()
        .unwrap();

    let req = SearchRequest::builder("rio")
        .kind(AssetKind::Equity)
        .build()
        .unwrap();
    let out = borsa.search(req).await.unwrap();

    let results = out.response.unwrap().results;
    // RIO should come from NYSE due to symbol override; other symbols preserved.
    let rio = results.iter().find(|r| r.symbol.as_str() == "RIO").unwrap();
    assert_eq!(rio.exchange, Exchange::try_from_str("NYSE").ok());
}

#[tokio::test]
async fn search_respects_exchange_priority_kind_only() {
    let c1 = m_search(
        "c1",
        vec![SearchResult {
            symbol: Symbol::new("DUAL").unwrap(),
            name: Some("Dual C1".into()),
            exchange: Exchange::try_from_str("NYSE").ok(),
            kind: AssetKind::Equity,
        }],
    );
    let c2 = m_search(
        "c2",
        vec![SearchResult {
            symbol: Symbol::new("DUAL").unwrap(),
            name: Some("Dual C2".into()),
            exchange: Exchange::try_from_str("LSE").ok(),
            kind: AssetKind::Equity,
        }],
    );

    let policy = RoutingPolicyBuilder::new()
        .exchanges_for_kind(
            AssetKind::Equity,
            &[
                Exchange::try_from_str("LSE").unwrap(),
                Exchange::try_from_str("NYSE").unwrap(),
            ],
        )
        .build();

    let borsa = Borsa::builder()
        .with_connector(c1.clone())
        .with_connector(c2.clone())
        .routing_policy(policy)
        .build()
        .unwrap();

    let req = SearchRequest::builder("dual")
        .kind(AssetKind::Equity)
        .build()
        .unwrap();
    let out = borsa.search(req).await.unwrap();
    let results_vec = out.response.unwrap().results;
    let dual = results_vec
        .iter()
        .find(|r| r.symbol.as_str() == "DUAL")
        .unwrap();
    assert_eq!(dual.exchange, Exchange::try_from_str("LSE").ok());
}

#[tokio::test]
async fn search_infers_kind_from_results_when_request_omits_it() {
    let c1 = m_search(
        "c1",
        vec![SearchResult {
            symbol: Symbol::new("DUAL").unwrap(),
            name: Some("Dual NYSE".into()),
            exchange: Exchange::try_from_str("NYSE").ok(),
            kind: AssetKind::Equity,
        }],
    );
    let c2 = m_search(
        "c2",
        vec![SearchResult {
            symbol: Symbol::new("DUAL").unwrap(),
            name: Some("Dual LSE".into()),
            exchange: Exchange::try_from_str("LSE").ok(),
            kind: AssetKind::Equity,
        }],
    );

    let policy = RoutingPolicyBuilder::new()
        .exchanges_for_kind(
            AssetKind::Equity,
            &[
                Exchange::try_from_str("LSE").unwrap(),
                Exchange::try_from_str("NYSE").unwrap(),
            ],
        )
        .build();

    let borsa = Borsa::builder()
        .with_connector(c1.clone())
        .with_connector(c2.clone())
        .routing_policy(policy)
        .build()
        .unwrap();

    let req = SearchRequest::builder("dual").build().unwrap();
    let out = borsa.search(req).await.unwrap();
    let results_vec = out.response.unwrap().results;
    let dual = results_vec
        .iter()
        .find(|r| r.symbol.as_str() == "DUAL")
        .unwrap();
    assert_eq!(dual.exchange, Exchange::try_from_str("LSE").ok());
}

#[tokio::test]
async fn search_unknown_exchange_ranks_last() {
    let c1 = m_search(
        "c1",
        vec![SearchResult {
            symbol: Symbol::new("UNK").unwrap(),
            name: Some("Unknown EX".into()),
            exchange: None,
            kind: AssetKind::Equity,
        }],
    );
    let c2 = m_search(
        "c2",
        vec![SearchResult {
            symbol: Symbol::new("UNK").unwrap(),
            name: Some("Known EX".into()),
            exchange: Exchange::try_from_str("NASDAQ").ok(),
            kind: AssetKind::Equity,
        }],
    );

    let policy = RoutingPolicyBuilder::new()
        .exchanges_for_kind(
            AssetKind::Equity,
            &[Exchange::try_from_str("NASDAQ").unwrap()],
        )
        .build();

    let borsa = Borsa::builder()
        .with_connector(c1.clone())
        .with_connector(c2.clone())
        .routing_policy(policy)
        .build()
        .unwrap();

    let req = SearchRequest::builder("unk")
        .kind(AssetKind::Equity)
        .build()
        .unwrap();
    let out = borsa.search(req).await.unwrap();
    let results_vec = out.response.unwrap().results;
    let unk = results_vec
        .iter()
        .find(|r| r.symbol.as_str() == "UNK")
        .unwrap();
    assert_eq!(unk.exchange, Exchange::try_from_str("NASDAQ").ok());
}
