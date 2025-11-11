
# borsa

[![Crates.io](https://img.shields.io/crates/v/borsa)](https://crates.io/crates/borsa)
[![Docs.rs](https://docs.rs/borsa/badge.svg)](https://docs.rs/borsa)
[![Downloads](https://img.shields.io/crates/d/borsa)](https://crates.io/crates/borsa)
[![License](https://img.shields.io/crates/l/borsa)](LICENSE)

**The unified, intelligent, and resilient financial data toolkit for Rust.**

## Overview

`borsa` provides a high-level, asynchronous API for fetching market and financial data from multiple sources. Instead of juggling different client libraries and data formats, `borsa` offers a single, consistent interface that intelligently routes requests across multiple data providers.

## Features

- **Pluggable Architecture**: Add multiple data connectors and let `borsa` automatically choose the best one
- **Intelligent Fallback**: If one provider fails, automatically try the next one
- **Smart Data Merging**: Combine data from multiple sources for more complete datasets
- **High Performance**: Async/await with efficient concurrent requests
- **Asset-Specific Routing**: Configure different providers for different asset types
- **Rich Data Types**: Quotes, historical data, fundamentals, options, news, and more

## Installation

Add `borsa` and a connector to your `Cargo.toml`:

```toml
[dependencies]
borsa = "0.3.0"
borsa-yfinance = "0.3.0"
tokio = { version = "1", features = ["full"] }
```

## Quickstart

```rust
use std::sync::Arc;
use borsa::Borsa;
use borsa_core::{AssetKind, Instrument};
use borsa_yfinance::YfConnector;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let yf = Arc::new(YfConnector::new_default());
    let borsa = Borsa::builder().with_connector(yf).build()?;

    let aapl = Instrument::from_symbol("AAPL", AssetKind::Equity)?;
    let q = borsa.quote(&aapl).await?;
    if let Some(price) = &q.price {
        println!(
            "{} last price: {}",
            q.symbol.as_str(),
            price.format()
        );
    }
    Ok(())
}
```

## Usage

- Simple quote: see the runnable example `borsa/examples/01_simple_quote.rs`.

## Concepts

### Middleware (quota-aware wrappers)

See `borsa/examples/24_quota_middleware.rs` for a runnable demonstration of `QuotaAwareConnector`.

#### Error handling behavior

- Provider messages that look like rate limits (e.g., contain "429", "rate limit", "too many requests") are normalized by the wrapper to `BorsaError::RateLimitExceeded`.
- When the quota is exhausted, the wrapper returns `BorsaError::QuotaExceeded { remaining, reset_in_ms }`.
- The router may temporarily blacklist a provider after long-window `QuotaExceeded` until the reset time, while transient per-slice blocks (from `EvenSpreadHourly`) do not trigger long-term blacklist and allow fallback.

## Observability (optional)

See workspace observability guidance in the root README: https://github.com/borsaorg/borsa/blob/main/README.md#observability-tracing

### Connectors

See the workspace for available connectors (e.g., `borsa-yfinance`).

### Instruments

An `Instrument` represents a financial asset. See `borsa/examples/03_search.rs` for creation and search basics.

### Priority Configuration

See routing policy examples `borsa/examples/12_per_symbol_priority.rs` and `borsa/examples/15_routing_policy_exchange_and_strict.rs`.

## Data Types

- Quotes: `borsa/examples/01_simple_quote.rs`
- Info snapshot: `borsa/examples/07_financial_snapshot.rs`
- History: `borsa/examples/02_history_merge.rs`
- Fundamentals: `borsa/examples/06_fundamentals_deep_dive.rs`
- Options: `borsa/examples/05_options_chain.rs`
- Analysis: `borsa/examples/10_analyst_recommendations.rs`, `borsa/examples/04_price_target.rs`
- News: `borsa/examples/19_news.rs`

## DataFrames (paft integration)

Enable the `dataframe` feature to use `.to_dataframe()` on returned types. See `borsa/examples/23_dataframe.rs`.

## Advanced Features

- Bulk download: `./examples/21_download_builder.rs`
- Resampling: `./examples/08_history_resampling.rs`
- Merge strategies: `./examples/14_merge_strategies.rs`
- Multi-quote: `./examples/22_multi_quotes.rs`
- Streaming quotes/options/candles: `./examples/17_streaming.rs`

## Architecture

See the workspace layout and ecosystem overview in the root README: https://github.com/borsaorg/borsa/blob/main/README.md#workspace-layout

### Building Custom Connectors

See the `borsa-core` crate documentation for role traits and capability accessors.

## Examples

See the latest runnable examples in `borsa/examples/`.

## Contributing

We welcome contributions! Please see our [Contributing Guide](https://github.com/borsaorg/borsa/blob/main/CONTRIBUTING.md) and our [Code of Conduct](https://github.com/borsaorg/borsa/blob/main/CODE_OF_CONDUCT.md).

For building, testing, and examples, see the root README: https://github.com/borsaorg/borsa/blob/main/README.md

## License

This project is licensed under the MIT License - see the [LICENSE](https://github.com/borsaorg/borsa/blob/main/LICENSE) file for details.

## Acknowledgments

- Yahoo Finance for providing free market data
- The Rust community for building amazing async tools

---

**Ready to build something amazing with financial data?** Start with `borsa` today!
