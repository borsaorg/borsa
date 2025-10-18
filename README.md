# borsa

High-level, pluggable market data API for Rust. This monorepo contains the core types and traits, the router/orchestrator, and the officially supported provider connector.

## Workspace layout

- `borsa-core`: shared types, errors, and the `BorsaConnector` trait
- `borsa`: high-level router that merges/prioritizes multiple connectors
- `borsa-yfinance`: Yahoo Finance connector (no API key required)
- `examples/`: self-contained example programs demonstrating common workflows

For crate-specific usage of the high-level client, see [borsa/README.md](https://github.com/borsaorg/borsa/blob/main/borsa/README.md).

## Install (as a user of the library)

Add the crates you need to your project:

```toml
[dependencies]
borsa = "0.1.0"
borsa-yfinance = "0.1.0"

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

## Router configuration highlights

- Prefer adjusted history when merging overlaps:

```rust
let borsa = Borsa::builder()
    .prefer_adjusted_history(true)
    .build()?;
```

- Resample merged history (daily or weekly):

```rust
use borsa::Resampling;
let borsa = Borsa::builder()
    .resampling(Resampling::Weekly)
    .build()?;
```

- Per-kind or per-symbol connector priority:

```rust
use borsa_core::AssetKind;
let yf = Arc::new(YfConnector::new_default());
let av = Arc::new(AvConnector::new_with_key("..."));

let borsa = Borsa::builder()
    .with_connector(yf.clone())
    .with_connector(av.clone())
    .prefer_for_kind(AssetKind::Equity, &[yf.clone(), av.clone()])
    .prefer_symbol("AAPL", &[av]) // overrides kind preference
    .build()?;
```

## Examples

Browse `examples/examples/` for end-to-end samples (quotes, history, fundamentals, options, news, ESG, analysis). These are small, copyable programs you can adapt in your own project.

### Running examples locally (CI-safe)

Examples dynamically select the connector at runtime via the `BORSA_EXAMPLES_USE_MOCK` environment variable:

- When set, examples use the deterministic `borsa-mock::MockConnector` (no network access)
- When unset, examples use the live `borsa-yfinance::YfConnector`

Run all examples against the live API (prints example outputs):

```bash
just examples
```

Run all example checks locally using the mock (CI-safe):

```bash
just examples-mock
```

Run a specific example with the mock:

```bash
BORSA_EXAMPLES_USE_MOCK=1 cargo run -p borsa-examples --example 01_simple_quote
```

Run a specific example against live Yahoo Finance:

```bash
cargo run -p borsa-examples --example 01_simple_quote
```

## Developing locally

- Build everything: `cargo build --workspace`
- Run tests: `just test`
- Lint: `just lint`
- Format: `just fmt`

## License and conduct

- License: MIT (see [LICENSE](https://github.com/borsaorg/borsa/blob/main/LICENSE))
- Participation: see [CODE_OF_CONDUCT.md](https://github.com/borsaorg/borsa/blob/main/CODE_OF_CONDUCT.md)

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](https://github.com/borsaorg/borsa/blob/main/CONTRIBUTING.md) for setup, workflow, testing, and how to implement new connectors.
