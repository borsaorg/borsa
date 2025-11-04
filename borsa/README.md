
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

## Usage

- Simple quote: see the runnable example `./examples/01_simple_quote.rs`.

## Concepts

### Middleware (quota-aware wrappers)

See `./examples/24_quota_middleware.rs` for a runnable demonstration of `QuotaAwareConnector`.

#### Error handling behavior

- Provider messages that look like rate limits (e.g., contain "429", "rate limit", "too many requests") are normalized by the wrapper to `BorsaError::RateLimitExceeded`.
- When the quota is exhausted, the wrapper returns `BorsaError::QuotaExceeded { remaining, reset_in_ms }`.
- The router may temporarily blacklist a provider after long-window `QuotaExceeded` until the reset time, while transient per-slice blocks (from `EvenSpreadHourly`) do not trigger long-term blacklist and allow fallback.

## Observability (optional)

See `./examples/00_tracing.rs` for a runnable tracing setup. Enable the `tracing` feature on `borsa` when needed.

### Connectors

See the workspace for available connectors (e.g., `borsa-yfinance`).

### Instruments

An `Instrument` represents a financial asset. See `./examples/03_search.rs` for creation and search basics.

### Priority Configuration

See routing policy examples `./examples/12_per_symbol_priority.rs` and `./examples/15_routing_policy_exchange_and_strict.rs`.

## Data Types

- Quotes: `./examples/01_simple_quote.rs`
- Info snapshot: `./examples/07_financial_snapshot.rs`
- History: `./examples/02_history_merge.rs`
- Fundamentals: `./examples/06_fundamentals_deep_dive.rs`
- Options: `./examples/05_options_chain.rs`
- Analysis: `./examples/10_analyst_recommendations.rs`, `./examples/04_price_target.rs`
- News: `./examples/19_news.rs`

## DataFrames (paft integration)

Enable the `dataframe` feature to use `.to_dataframe()` on returned types. See `./examples/23_dataframe.rs`.

## Advanced Features

- Bulk download: `./examples/21_download_builder.rs`
- Resampling: `./examples/08_history_resampling.rs`
- Merge strategies: `./examples/14_merge_strategies.rs`
- Multi-quote: `./examples/22_multi_quotes.rs`
- Streaming: `./examples/17_streaming.rs`

## Architecture

### The Borsa Ecosystem

- **`borsa`**: High-level client library (this crate)
- **`borsa-core`**: Core traits and types for building connectors
- **`borsa-yfinance`**: Yahoo Finance connector

### Building Custom Connectors

See the `borsa-core` crate documentation for role traits and capability accessors.

## Examples

See the latest runnable examples in `./examples/`.

## Contributing

We welcome contributions! Please see our [Contributing Guide](https://github.com/borsaorg/borsa/blob/main/CONTRIBUTING.md) and our [Code of Conduct](https://github.com/borsaorg/borsa/blob/main/CODE_OF_CONDUCT.md).

### Building from Source

```bash
git clone https://github.com/borsaorg/borsa.git
cd borsa
cargo build --workspace
```

### Running Tests

```bash
cargo test --workspace
```

### Running Examples

```bash
cd examples
cargo run --example 01_simple_quote
```

## License

This project is licensed under the MIT License - see the [LICENSE](https://github.com/borsaorg/borsa/blob/main/LICENSE) file for details.

## Acknowledgments

- Yahoo Finance for providing free market data
- The Rust community for building amazing async tools

---

**Ready to build something amazing with financial data?** Start with `borsa` today!
