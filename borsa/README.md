
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
borsa = "0.1.0"
borsa-yfinance = "0.1.0"
tokio = { version = "1", features = ["full"] }
```

## Usage

### Fetch your first quote

```rust
use borsa::Borsa;
use borsa_core::{AssetKind, Instrument};
use borsa_yfinance::YfConnector;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a connector and build the client
    let yf = Arc::new(YfConnector::new_default());
    let borsa = Borsa::builder().with_connector(yf).build();

    // Define the instrument (validated + uppercased automatically)
    let aapl = Instrument::from_symbol("AAPL", AssetKind::Equity)?;

    // Fetch the quote
    let quote = borsa.quote(&aapl).await?;
    if let Some(price) = &quote.price {
        println!("{} last: {}", quote.symbol.as_str(), price.format());
    }

    Ok(())
}
```

## Concepts

### Connectors

Connectors are plugins that fetch data from specific providers. `borsa` comes with:

- **`borsa-yfinance`**: Yahoo Finance connector (free, no API key required)

### Instruments

An `Instrument` represents a financial asset:

```rust
use borsa_core::{AssetKind, Instrument};

// Stocks
let aapl = Instrument::from_symbol("AAPL", AssetKind::Equity)
    .expect("valid symbol");
let tsla = Instrument::from_symbol("TSLA", AssetKind::Equity)
    .expect("valid symbol");

// Cryptocurrencies
let btc = Instrument::from_symbol("BTC-USD", AssetKind::Crypto)
    .expect("valid symbol");

// ETFs
let spy = Instrument::from_symbol("SPY", AssetKind::Equity)
    .expect("valid symbol");
```

### Priority Configuration

Configure which connectors to use for different assets:

```rust
let borsa = Borsa::builder()
    .with_connector(yf_connector.clone())
    .with_connector(alpha_vantage_connector.clone())
    // Prefer Alpha Vantage for crypto (type-safe, ergonomic API)
    .prefer_for_kind(AssetKind::Crypto, &[alpha_vantage_connector.clone(), yf_connector.clone()])
    // Use specific connector for TSLA
    .prefer_symbol("TSLA", &[alpha_vantage_connector, yf_connector])
    .build();
```

## Data Types

### Quotes & Market Data

```rust
// Get live quote
let quote = borsa.quote(&aapl).await?;
if let Some(price) = &quote.price {
    println!("{} price: {}", quote.symbol.as_str(), price.format());
}

// Get comprehensive info (snapshot + warnings)
let report = borsa.info(&aapl).await?;
if let Some(info) = report.info {
    if let Some(price) = &info.last {
        println!("{} last: {}", info.symbol.as_str(), price.format());
    }
    println!("Market state: {}", info.market_state.map_or("N/A".into(), |s| s.to_string()));
    println!("Warnings: {}", report.warnings.join(", "));
}
```

### Historical Data

```rust
use borsa_core::{HistoryRequest, Range, Interval};

// Get 6 months of daily data
let req = HistoryRequest::try_from_range(Range::M6, Interval::D1)?;
let history = borsa.history(&aapl, req.clone()).await?;

println!("Fetched {} candles", history.candles.len());
if let Some(last) = history.candles.last() {
    println!("Latest close: {}", last.close.format());
}

// Get data with attribution (see which connector provided each piece)
let (history, attribution) = borsa.history_with_attribution(&aapl, req).await?;
for (connector, span) in attribution.spans {
    println!("{}: {} -> {}", connector, span.start, span.end);
}
```

### Fundamentals

```rust
// Income Statement
let income = borsa.income_statement(&aapl, true).await?; // quarterly
if let Some(latest) = income.first() {
    if let Some(revenue) = &latest.total_revenue {
        println!("Revenue: {}", revenue.format());
    }
}

// Balance Sheet
let balance = borsa.balance_sheet(&aapl, true).await?;
if let Some(latest) = balance.first() {
    if let Some(assets) = &latest.total_assets {
        println!("Total Assets: {}", assets.format());
    }
}

// Cash Flow
let cashflow = borsa.cashflow(&aapl, true).await?;
if let Some(latest) = cashflow.first() {
    if let Some(fcf) = &latest.free_cash_flow {
        println!("Free Cash Flow: {}", fcf.format());
    }
}
```

### Options Data

```rust
// Get available expiration dates
let expirations = borsa.options_expirations(&aapl).await?;
println!("Found {} expiration dates", expirations.len());

// Get option chain for nearest expiration
if let Some(&next_expiry) = expirations.first() {
    let chain = borsa.option_chain(&aapl, Some(next_expiry)).await?;
    println!("Calls: {}, Puts: {}", chain.calls.len(), chain.puts.len());
}
```

### Analysis & Recommendations

```rust
// Analyst recommendations
let recs = borsa.recommendations(&aapl).await?;
let summary = borsa.recommendations_summary(&aapl).await?;
println!("Mean recommendation: {}", summary.mean.unwrap_or(0.0));

// Price targets
let target = borsa.analyst_price_target(&aapl).await?;
let mean = target.mean.as_ref().map(|m| m.format()).unwrap_or_else(|| "N/A".into());
let low = target.low.as_ref().map(|m| m.format()).unwrap_or_else(|| "N/A".into());
let high = target.high.as_ref().map(|m| m.format()).unwrap_or_else(|| "N/A".into());
println!("Target: {mean} (low: {low}, high: {high})");
```

### News & Events

```rust
use borsa_core::types::NewsRequest;

// Get recent news
let news_req = NewsRequest::default();
let news = borsa.news(&aapl, news_req).await?;
for article in news.iter().take(5) {
    println!("{}: {}", article.title, article.publisher.as_deref().unwrap_or(""));
}

// Get upcoming events
let calendar = borsa.calendar(&aapl).await?;
for ts in calendar.earnings_dates.iter().take(3) {
    println!("Earnings at: {}", ts);
}
```

## 🔧 Advanced Features

### Bulk Operations

Download data for multiple instruments efficiently:

```rust
let instruments = [
    Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid symbol"),
    Instrument::from_symbol("GOOGL", AssetKind::Equity).expect("valid symbol"),
    Instrument::from_symbol("MSFT", AssetKind::Equity).expect("valid symbol"),
];

let summary = borsa.download()
    .instruments(&instruments)?
    .range(Range::Y1)
    .interval(Interval::D1)
    .run()
    .await?;

if let Some(report) = summary.response {
    for (symbol, history) in report.history {
        println!("{}: {} candles", symbol.as_str(), history.candles.len());
    }
}
```

### Automatic Resampling

Configure automatic data resampling:

```rust
use borsa::Resampling;

let borsa = Borsa::builder()
    .with_connector(yf_connector)
    // Always convert to daily bars
    .resampling(Resampling::Daily)
    // Or convert to weekly bars
    // .resampling(Resampling::Weekly)
    .build();
```

### History Merge Strategy

Control how historical data is fetched from multiple providers:

```rust
use borsa::MergeStrategy;

let borsa = Borsa::builder()
    .with_connector(yf_connector)
    .with_connector(alpha_vantage_connector)
    // Deep merge: fetch from all providers and merge data (default)
    .merge_history_strategy(MergeStrategy::Deep)
    // Or fallback: stop at first provider with data (more economical)
    // .merge_history_strategy(MergeStrategy::Fallback)
    .build();
```

**Merge Strategies:**

- **`Deep`** (default): Fetches from all eligible providers concurrently and merges their data. This produces the most complete dataset by backfilling gaps from lower-priority providers, but uses more API calls.
- **`Fallback`**: Iterates through providers sequentially and stops as soon as one returns a non-empty dataset. This is more economical for API rate limits but may miss data from lower-priority providers.

### Multi-Quote Fetching

Get quotes for multiple instruments efficiently:

```rust
let instruments = [
    Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid symbol"),
    Instrument::from_symbol("GOOGL", AssetKind::Equity).expect("valid symbol"),
    Instrument::from_symbol("BTC-USD", AssetKind::Crypto).expect("valid symbol"),
];

let (quotes, failures) = borsa.quotes(&instruments).await?;
for q in quotes {
    if let Some(price) = &q.price {
        println!("{}: {}", q.symbol.as_str(), price.format());
    }
}
if !failures.is_empty() {
    for (inst, err) in failures {
        eprintln!("failed: {} -> {}", inst.symbol().as_str(), err);
    }
}
```

## 🏗️ Architecture

### The Borsa Ecosystem

- **`borsa`**: High-level client library (this crate)
- **`borsa-core`**: Core traits and types for building connectors
- **`borsa-yfinance`**: Yahoo Finance connector

### Building Custom Connectors

Create your own connector by implementing capability role traits and advertising them via `BorsaConnector`:

```rust
use borsa_core::{BorsaConnector, Instrument, Quote, BorsaError, Interval, AssetKind};
use async_trait::async_trait;

pub struct MyConnector;

#[async_trait]
impl borsa_core::connector::QuoteProvider for MyConnector {
    async fn quote(&self, inst: &Instrument) -> Result<Quote, BorsaError> {
        // Fetch quote from your backend
        let price = borsa_core::Money::from_canonical_str(
            "123.45",
            borsa_core::Currency::Iso(borsa_core::IsoCurrency::USD),
        )
        .expect("static demo price");
        Ok(Quote {
            symbol: inst.symbol().clone(),
            shortname: None,
            price: Some(price),
            previous_close: None,
            exchange: None,
            market_state: None,
        })
    }
}

impl borsa_core::connector::BorsaConnector for MyConnector {
    fn name(&self) -> &'static str { "my-connector" }
    fn as_quote_provider(&self) -> Option<&dyn borsa_core::connector::QuoteProvider> { Some(self) }
    fn as_history_provider(&self) -> Option<&dyn borsa_core::connector::HistoryProvider> { Some(self) }
}
```

## 📖 Examples

Check out the [examples package](../examples/examples/) for comprehensive usage examples:

- `01_simple_quote.rs` - Basic quote fetching
- `02_history_merge.rs` - Historical data with multiple sources
- `03_search.rs` - Symbol search
- `04_price_target.rs` - Analyst price target
- `05_options_chain.rs` - Options data
- `06_fundamentals_deep_dive.rs` - Financial statements
- `07_financial_snapshot.rs` - Aggregated info snapshot
- `08_history_resampling.rs` - Resampling and cadence control
- `09_stock_comparison.rs` - Multi-symbol comparison
- `10_analyst_recommendations.rs` - Analyst data
- `11_upcoming_events.rs` - Calendar/events
- `12_per_symbol_priority.rs` - Per-symbol provider priority

## 🤝 Contributing

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

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](https://github.com/borsaorg/borsa/blob/main/LICENSE) file for details.

## 🙏 Acknowledgments

- Yahoo Finance for providing free market data
- Alpha Vantage for their comprehensive financial APIs
- The Rust community for building amazing async tools

---

**Ready to build something amazing with financial data?** Start with `borsa` today! 🚀
