# borsa-yfinance

Yahoo Finance connector for the borsa ecosystem. This crate is both a ready-to-use provider and a reference implementation for building custom connectors.

[![Crates.io](https://img.shields.io/crates/v/borsa-yfinance)](https://crates.io/crates/borsa-yfinance)
[![Docs.rs](https://docs.rs/borsa-yfinance/badge.svg)](https://docs.rs/borsa-yfinance)
[![Downloads](https://img.shields.io/crates/d/borsa-yfinance)](https://crates.io/crates/borsa-yfinance)
[![License](https://img.shields.io/crates/l/borsa-yfinance)](https://github.com/borsaorg/borsa/blob/main/LICENSE)

## Overview

`borsa-yfinance` implements `borsa-core::BorsaConnector` using `yfinance-rs` under the hood. It covers a wide set of capabilities: quotes, history, search, profile, fundamentals, options, analysis, holders, sustainability, and news, and can be used as a reference when building a connector.

Use it directly, or follow its patterns to build your own connector.

## Install

```toml
[dependencies]
borsa-yfinance = "0.3.0"
borsa-core = "0.3.0"
```

## Quick start

```rust
use borsa_yfinance::YfConnector;
use borsa_core::{connector::QuoteProvider, AssetKind, Instrument};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let yf = borsa_yfinance::YfConnector::rate_limited().build();
    let aapl = Instrument::from_symbol("AAPL", AssetKind::Equity)?;
    let q = yf.quote(&aapl).await?;
    if let Some(price) = &q.price {
        println!("{} price: {}", q.symbol.as_str(), price.format());
    }
    Ok(())
}
```

## Using YfConnector in the router

## Observability

Enable the `tracing` feature to emit spans for all public provider endpoints (quotes, history, search, profile, fundamentals, options, analysis, holders, ESG, news, streaming):

```toml
[dependencies]
borsa-yfinance = { version = "0.3.0", features = ["tracing"] }
```

Run with the example subscriber setup:

```bash
RUST_LOG=info,borsa=trace,borsa_yfinance=trace \
  cargo run -p borsa --example 00_tracing \
  --features "borsa/tracing borsa-yfinance/tracing"
```

```rust
use borsa::{Borsa};
use borsa_yfinance::YfConnector;
use borsa_core::{connector::QuoteProvider, AssetKind, Currency, Instrument, Money, Symbol};
use std::sync::Arc;

let yf = borsa_yfinance::YfConnector::rate_limited().build();
let borsa = Borsa::builder().with_connector(yf).build()?;
let inst = Instrument::from_symbol("MSFT", AssetKind::Equity)?;
let quote = borsa.quote(&inst).await?;
```

## Designing a connector: the YF blueprint

1. Define a small set of adapter traits to wrap the SDK

```rust
#[async_trait]
pub trait YfQuotes { async fn fetch(&self, symbols: &[String]) -> Result<Vec<yf::core::Quote>, BorsaError>; }
#[async_trait]
pub trait YfHistory { async fn fetch_full(&self, symbol: &str, req: yf::core::services::HistoryRequest) -> Result<yf::HistoryResponse, BorsaError>; }
// ... YfSearch, YfProfile, YfFundamentals, YfOptions, YfAnalysis, YfHolders, YfEsg, YfNews
```

2. Provide an adapter that holds the client once and implements all adapters

```rust
#[derive(Clone)]
pub struct RealAdapter { client: yf::YfClient }
impl RealAdapter { pub fn new_default() -> Self { Self { client: yf::YfClient::default() } } }
```

3. Expose test adapters via closures so unit tests don’t need network access

```rust
impl dyn YfQuotes { pub fn from_fn<F>(f: F) -> Arc<dyn YfQuotes> where F: Send + Sync + 'static + Fn(Vec<String>) -> Result<Vec<yf::core::Quote>, BorsaError> { /* ... */ } }
```

4. Return the native paft types (`Symbol`, `Money`, domain enums) directly from adapters.

5. Delegate capability traits and advertise them via `BorsaConnector::as_*_provider`.

```rust
#[async_trait]
impl QuoteProvider for YfConnector {
    async fn quote(&self, instrument: &Instrument) -> Result<Quote, BorsaError> {
        // call adapter + normalise errors
    }
}

impl BorsaConnector for YfConnector {
    fn name(&self) -> &'static str { "borsa-yfinance" }

    fn as_quote_provider(&self) -> Option<&dyn QuoteProvider> {
        Some(self)
    }
    // advertise other capabilities similarly
}
```

## Capability matrix

This connector advertises and implements the following capabilities:

- Quotes, History, Search, Profile
- Fundamentals (earnings, statements), Options
- Analysis (recommendations, price targets), Holders
- ESG (sustainability scores), News

## History intervals

Native intervals returned by `supported_history_intervals`:

- 1m, 2m, 5m, 15m, 30m, 60m, 90m, 1d, 5d, 1w, 1mo, 3mo

The orchestrator may resample as needed (e.g., auto-subdaily->daily, weekly).

## Error mapping

Errors from `yfinance-rs` are converted to `BorsaError::connector("borsa-yfinance", message)` to provide consistent, debuggable failures in multi-provider flows. Missing symbols surface as `BorsaError::NotFound{ .. }` where relevant via router logic.

## Testing strategy

- Unit tests use closure-based test adapters to inject precise responses and errors
- Conversion tests validate field-by-field mapping is stable
- Capability tests ensure flags correctly reflect implemented adapters

Run:

```bash
cargo test -p borsa-yfinance | cat
```

## Contributing guidelines for connector authors

- Keep the public connector small; put IO and state in an adapter layer
- Surface exact native intervals; leave planning/resampling to the router
- Implement only supported endpoints and let defaults return `unsupported`
- Prefer deterministic, pure conversions; avoid IO in mapping code
- Accurately reflect capabilities; the router depends on them for routing
- Provide test adapters so contributors can write focused tests without network

## Building the connector for testability (patterns and examples)

> **Feature flag:** the lightweight adapter helpers (`CloneArcAdapters`, `YfQuotes::from_fn`, etc.)
> are gated behind the optional `test-adapters` feature. Enable it in `Cargo.toml` (for example,
> `borsa-yfinance = { version = "x.y", features = ["test-adapters"] }`) or on the command line with
> `cargo test --features borsa-yfinance/test-adapters`.

The design here intentionally separates the public connector from IO so you can write fast, deterministic tests with zero network.

### Pattern A: Quotes-only unit test (no router)

```rust
use std::sync::Arc;
use borsa_yfinance::YfConnector;
use borsa_yfinance::adapter::{CloneArcAdapters, YfQuotes};
use borsa_core::{connector::QuoteProvider, AssetKind, Currency, Instrument, Money, Symbol};

// 1) Create a minimal adapter that exposes only quotes
struct QuotesOnlyAdapter { quotes: Arc<dyn YfQuotes> }
impl CloneArcAdapters for QuotesOnlyAdapter {
    fn clone_arc_quotes(&self) -> Arc<dyn YfQuotes> { self.quotes.clone() }
}

#[tokio::test]
async fn quote_smoke_test() {
    // 2) Provide a closure-based quote implementation (no network)
    let quotes = <dyn YfQuotes>::from_fn(|symbols| {
        assert_eq!(symbols, vec!["AAPL".to_string()]);
        let price = Money::from_canonical_str(
            "190.0",
            Currency::Iso(borsa_core::IsoCurrency::USD),
        )
        .unwrap();
        let previous = Money::from_canonical_str(
            "189.5",
            Currency::Iso(borsa_core::IsoCurrency::USD),
        )
        .unwrap();
        Ok(vec![yfinance_rs::core::Quote {
            symbol: Symbol::new("AAPL").unwrap(),
            shortname: Some("Apple".into()),
            price: Some(price),
            previous_close: Some(previous),
            exchange: None,
            market_state: None,
        }])
    });

    // 3) Build the connector from the adapter
    let yf = YfConnector::from_adapter(QuotesOnlyAdapter { quotes });

    // 4) Exercise the API
    let aapl = Instrument::from_symbol("AAPL", AssetKind::Equity).unwrap();
    let q = yf.quote(&aapl).await.unwrap();
    assert_eq!(q.symbol.as_str(), "AAPL");
    assert_eq!(q.price.unwrap().format(), "190.0 USD");
}
```

### Pattern B: Search unit test using the adapter helpers

```rust
use std::sync::Arc;
use borsa_yfinance::YfConnector;
use borsa_yfinance::adapter::{CloneArcAdapters, YfSearch};
use borsa_core::{AssetKind, SearchRequest, SearchResponse, SearchResult, Symbol};

struct SearchOnlyAdapter { search: Arc<dyn YfSearch> }
impl CloneArcAdapters for SearchOnlyAdapter {
    fn clone_arc_search(&self) -> Arc<dyn YfSearch> { self.search.clone() }
}

#[tokio::test]
async fn search_returns_symbols() {
    let search = <dyn YfSearch>::from_fn(|query| {
        assert_eq!(query, "Apple");
        Ok(SearchResponse {
            results: vec![
                SearchResult {
                    symbol: Symbol::new("AAPL").unwrap(),
                    name: Some("Apple Inc.".into()),
                    exchange: None,
                    kind: AssetKind::Equity,
                },
                SearchResult {
                    symbol: Symbol::new("APPL34").unwrap(),
                    name: Some("Apple BDR".into()),
                    exchange: None,
                    kind: AssetKind::Equity,
                },
            ],
        })
    });

    let yf = YfConnector::from_adapter(SearchOnlyAdapter { search });
    let res = yf.search(SearchRequest::new("Apple").with_limit(2)).await.unwrap();
    assert_eq!(res.results.len(), 2);
    assert_eq!(res.results[0].symbol.as_str(), "AAPL");
}
```

### Pattern C: End-to-end router test with injected YF

```rust
use std::sync::Arc;
use borsa::Borsa;
use borsa_core::{Instrument, AssetKind};
use borsa_yfinance::YfConnector;
use borsa_yfinance::adapter::{CloneArcAdapters, YfQuotes};

struct QuotesOnlyAdapter { quotes: Arc<dyn YfQuotes> }
impl CloneArcAdapters for QuotesOnlyAdapter {
    fn clone_arc_quotes(&self) -> Arc<dyn YfQuotes> { self.quotes.clone() }
}

#[tokio::test]
async fn router_uses_injected_yf() {
    let quotes = <dyn YfQuotes>::from_fn(|symbols| Ok(vec![yfinance_rs::core::Quote {
        symbol: symbols[0].clone(),
        shortname: None,
        regular_market_price: Some(123.45),
        regular_market_previous_close: None,
        currency: None,
        exchange: None,
        market_state: None,
    }]))
    ;

    let yf = Arc::new(YfConnector::from_adapter(QuotesOnlyAdapter { quotes }));
    let borsa = Borsa::builder().with_connector(yf).build()?;

    let inst = Instrument::new("MSFT", AssetKind::Equity);
    let q = borsa.quote(&inst).await.unwrap();
    assert_eq!(q.symbol, "MSFT");
}
```

## License

MIT — see [LICENSE](https://github.com/borsaorg/borsa/blob/main/LICENSE)

## Disclaimer

This crate provides access to Yahoo Finance data. Ensure compliance with Yahoo’s terms of service.
