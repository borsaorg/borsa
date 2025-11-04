# borsa-core

Core types, traits, and utilities shared across the borsa financial data ecosystem.

[![Crates.io](https://img.shields.io/crates/v/borsa-core)](https://crates.io/crates/borsa-core)
[![Docs.rs](https://docs.rs/borsa-core/badge.svg)](https://docs.rs/borsa-core)
[![Downloads](https://img.shields.io/crates/d/borsa-core)](https://crates.io/crates/borsa-core)
[![License](https://img.shields.io/crates/l/borsa-core)](LICENSE)

## Overview

`borsa-core` provides the foundational building blocks for the borsa ecosystem, a unified interface for accessing financial market data from multiple providers. It defines common data structures, the connector trait for implementing data providers, and utilities for working with time series data.

## Features

### Core Components

- **`BorsaConnector` trait**: The main interface that all data providers must implement
- **Capability directory**: Accessors (`as_*_provider`) expose granular capability traits
- **Common data types**: Unified structures for quotes, candles, fundamentals, and more
- **Time series utilities**: Merging, resampling, and processing historical data

### Supported Data Types

- **Quotes**: Real-time and delayed price data
- **Historical data**: OHLCV candles with corporate actions (dividends, splits)
- **Fundamentals**: Income statements, balance sheets, cash flow statements
- **Profiles**: Company and fund information
- **Options**: Option chains and expiration data
- **Analysis**: Price targets, recommendations, upgrades/downgrades
- **Holders**: Institutional and insider holdings
- **ESG**: Environmental, social, and governance scores
- **News**: Financial news articles and events

### Asset Types

The library supports multiple asset classes:

- Equities (stocks)
- Cryptocurrencies
- Funds (ETFs, mutual funds)
- Indices
- Forex (foreign exchange)
- Bonds
- Commodities
- Derivatives

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
borsa-core = "0.3.0"
```

## Usage

### Basic Example

After adding `borsa-core` to your `Cargo.toml`, you can start with the following examples.

```rust
use borsa_core::{AssetKind, BorsaConnector, BorsaError, Instrument};

// Create an instrument (validated and canonicalized)
let instrument = Instrument::from_symbol("AAPL", AssetKind::Equity)?;

// Use with any connector that implements BorsaConnector
async fn get_quote(connector: &impl BorsaConnector, instrument: &Instrument) -> Result<(), BorsaError> {
    let provider = connector
        .as_quote_provider()
        .ok_or_else(|| BorsaError::unsupported("quote"))?;
    let quote = provider.quote(instrument).await?;
    if let Some(price) = &quote.price {
        println!("{}: {}", quote.symbol.as_str(), price.format());
    }
    Ok(())
}
```

### Working with Capabilities

```rust
use borsa_core::BorsaConnector;

fn check_support(connector: &impl BorsaConnector) {
    if connector.as_quote_provider().is_some() {
        println!("This connector supports real-time quotes");
    }
    if connector.as_history_provider().is_some() {
        println!("This connector supports historical data");
    }
    if connector.as_earnings_provider().is_some() {
        println!("This connector supports earnings");
    }
}
```

### Time Series Operations

```rust
use borsa_core::timeseries::{merge::merge_history, resample::resample_to_daily};

// Merge multiple HistoryResponse values in priority order
let merged = merge_history(vec![resp_a, resp_b, resp_c]);

// Resample arbitrary candles to daily bars
let daily_candles = resample_to_daily(candles)?;
```

## Architecture

### Connector Trait and Capabilities

`BorsaConnector` is a capability hub: providers implement granular role traits and advertise them via `as_*_provider` accessors on the connector. This keeps the core stable and enables mix-and-match features. Use `supports_kind(&AssetKind)` to declare which asset classes the connector can serve.

```rust
use borsa_core::connector::{BorsaConnector, QuoteProvider, HistoryProvider};

pub struct MyConnector;

#[async_trait]
impl QuoteProvider for MyConnector {
    async fn quote(&self, instrument: &Instrument) -> Result<Quote, BorsaError> {
        // ...
    }
}

#[async_trait]
impl HistoryProvider for MyConnector {
    async fn history(&self, instrument: &Instrument, req: HistoryRequest) -> Result<HistoryResponse, BorsaError> {
        // ...
    }
    fn supported_history_intervals(&self, _kind: AssetKind) -> &'static [Interval] { &[] }
}

impl BorsaConnector for MyConnector {
    fn name(&self) -> &'static str { "my-connector" }
    fn supports_kind(&self, kind: AssetKind) -> bool { matches!(kind, AssetKind::Equity) }
    fn as_quote_provider(&self) -> Option<&dyn QuoteProvider> { Some(self) }
    fn as_history_provider(&self) -> Option<&dyn HistoryProvider> { Some(self) }
}
```

## Documentation

- [API Documentation](https://docs.rs/borsa-core)
- Examples: see the workspace `examples/` package

## Related Crates

- `borsa`: High-level router/orchestrator
- `borsa-yfinance`: Yahoo Finance connector

## Contributing

Contributions are welcome! Please see our [Contributing Guide](https://github.com/borsaorg/borsa/blob/main/CONTRIBUTING.md) and our [Code of Conduct](https://github.com/borsaorg/borsa/blob/main/CODE_OF_CONDUCT.md). For major changes, please open an issue first to discuss what you would like to change.

## License

This project is licensed under the MIT License - see the [LICENSE](https://github.com/borsaorg/borsa/blob/main/LICENSE) file for details.
