# borsa-middleware

Reusable middleware for `borsa` connectors. Currently includes a quota-aware wrapper that enforces request budgets and normalizes common provider rate-limit errors.

## Install

```toml
[dependencies]
borsa = "0.2"
borsa-middleware = "0.2"
borsa-types = "0.2"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }

# For local testing/examples (uses fixtures, no network)
borsa-mock = { version = "0.2", optional = true }
```

## Quota-aware wrapper

`QuotaAwareConnector` wraps any `BorsaConnector` and enforces a budget over a sliding window. It also translates provider-specific rate-limit messages into a normalized `BorsaError::RateLimitExceeded`.

### Example: Wrap a connector with a daily budget

```rust,no_run
use std::sync::Arc;
use std::time::Duration;

use borsa::Borsa;
use borsa_core::{AssetKind, Instrument, BorsaConnector};
use borsa_middleware::QuotaAwareConnector;
use borsa_types::{QuotaConfig, QuotaConsumptionStrategy, QuotaState};

// Use the mock connector for CI-safe examples. Replace with a real connector in your app.
use borsa_mock::MockConnector;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1 call/day budget using the Unit strategy
    let cfg = QuotaConfig {
        limit: 1,
        window: Duration::from_secs(24 * 60 * 60),
        strategy: QuotaConsumptionStrategy::Unit,
    };
    let st = QuotaState { limit: cfg.limit, remaining: cfg.limit, reset_in: cfg.window };

    let inner: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());
    let wrapped = Arc::new(QuotaAwareConnector::new(inner, cfg, st));

    let borsa = Borsa::builder()
        .with_connector(wrapped)
        .build()?;

    let aapl = Instrument::from_symbol("AAPL", AssetKind::Equity)?;
    let _ = borsa.quote(&aapl).await?;           // allowed
    let err = borsa.quote(&aapl).await.unwrap_err(); // exceeds quota
    eprintln!("second call failed: {}", err);
    Ok(())
}
```

### Example: Smooth usage with `EvenSpreadHourly`

The `EvenSpreadHourly` strategy evenly spreads a daily budget across 24 slices (typically ~1h each for a 24h window). If the current slice is exhausted but the daily budget remains, calls are temporarily blocked with `QuotaExceeded { remaining > 0 }`. The orchestrator can then fall back to other providers.

```rust,no_run
use std::sync::Arc;
use std::time::Duration;

use borsa::Borsa;
use borsa_core::{AssetKind, Instrument, BorsaConnector};
use borsa_middleware::QuotaAwareConnector;
use borsa_types::{QuotaConfig, QuotaConsumptionStrategy, QuotaState};
use borsa_mock::MockConnector;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 24 calls/day -> ~1 per slice (hour)
    let cfg = QuotaConfig {
        limit: 24,
        window: Duration::from_secs(24 * 60 * 60),
        strategy: QuotaConsumptionStrategy::EvenSpreadHourly,
    };
    let st = QuotaState { limit: cfg.limit, remaining: cfg.limit, reset_in: cfg.window };

    let primary: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());
    let fallback: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());

    let primary_wrapped = Arc::new(QuotaAwareConnector::new(primary, cfg, st));

    let borsa = Borsa::builder()
        .with_connector(primary_wrapped)   // attempted first
        .with_connector(fallback)          // used when slice-blocked
        .build()?;

    let aapl = Instrument::from_symbol("AAPL", AssetKind::Equity)?;

    // First call in the slice is allowed on primary
    let _ = borsa.quote(&aapl).await?;
    // Second immediate call may hit the per-slice block and fall back to the next provider
    let _ = borsa.quote(&aapl).await?;
    Ok(())
}
```

### Error normalization

- Provider-specific messages that look like rate limits (e.g., contain "429", "rate limit", "too many requests") are mapped to `BorsaError::RateLimitExceeded` by the wrapper.
- When the quota budget is exhausted, the wrapper returns `BorsaError::QuotaExceeded { remaining, reset_in_ms }`.
- In the orchestrator, a long-window `QuotaExceeded` can trigger temporary provider blacklisting until the window resets; per-slice blocks from `EvenSpreadHourly` are treated as transient, allowing immediate fallback to other providers.

## Notes

- Thread-safe and cheap to clone via `Arc`.
- Strategy `Weighted` exists in `borsa-types` but is not yet implemented by this wrapper.

## See also

- Main crate: `borsa`
- Core traits and types: `borsa-core`, `borsa-types`
