# borsa-middleware

Reusable middleware for `borsa` connectors. Currently includes a quota-aware wrapper that enforces request budgets and normalizes common provider rate-limit errors.

## Install

```toml
[dependencies]
borsa = "0.3.0"
borsa-middleware = "0.3.0"
borsa-types = "0.3.0"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }

# For local testing/examples (uses fixtures, no network)
borsa-mock = { version = "0.3.0", optional = true }
```

## Quota-aware wrapper

`QuotaAwareConnector` wraps any `BorsaConnector` and enforces a budget over a sliding window. It also translates provider-specific rate-limit messages into a normalized `BorsaError::RateLimitExceeded`.

### Example: Wrap a connector with a daily budget

```rust,ignore
use std::sync::Arc;
use std::time::Duration;

use borsa::Borsa;
use borsa_core::{AssetKind, Instrument, BorsaConnector};
use borsa_middleware::QuotaAwareConnector;
use borsa_types::{QuotaConfig, QuotaConsumptionStrategy};

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
    let inner: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());
    let wrapped = Arc::new(QuotaAwareConnector::new(inner, cfg));

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

```rust,ignore
use std::sync::Arc;
use std::time::Duration;

use borsa::Borsa;
use borsa_core::{AssetKind, Instrument, BorsaConnector};
use borsa_middleware::QuotaAwareConnector;
use borsa_types::{QuotaConfig, QuotaConsumptionStrategy};
use borsa_mock::MockConnector;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 24 calls/day -> ~1 per slice (hour)
    let cfg = QuotaConfig {
        limit: 24,
        window: Duration::from_secs(24 * 60 * 60),
        strategy: QuotaConsumptionStrategy::EvenSpreadHourly,
    };
    let primary: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());
    let fallback: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());

    let primary_wrapped = Arc::new(QuotaAwareConnector::new(primary, cfg));

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

## Blacklist middleware

`BlacklistConnector` temporarily gates a provider after upstream rate-limit signals. It checks at the start of each call and returns `BorsaError::TemporarilyBlacklisted { reset_in_ms }` while the provider is blacklisted.

- Honors upstream `BorsaError::RateLimitExceeded { window_ms }` when available; otherwise uses a configured default duration.
- Internal fan-out calls flagged with `CallOrigin::Internal` bypass checks, so compositional requests don't poison the global budget.

### Example: Apply a 5-minute default blacklist window

```rust,ignore
use std::sync::Arc;
use std::time::Duration;

use borsa::Borsa;
use borsa_core::{AssetKind, Instrument, BorsaConnector};
use borsa_middleware::ConnectorBuilder;
use borsa_mock::MockConnector;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raw: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());

    let wrapped = ConnectorBuilder::new(raw)
        .with_blacklist(Duration::from_secs(300)) // default when upstream window is unknown
        .build()?;

    let borsa = Borsa::builder().with_connector(wrapped).build()?;

    let aapl = Instrument::from_symbol("AAPL", AssetKind::Equity)?;
    // First call may succeed; subsequent calls during blacklist window are blocked
    let _ = borsa.quote(&aapl).await;
    Ok(())
}
```

### Behavior

- Upstream errors mapped to `RateLimitExceeded` set the blacklist until the provider's window elapses.
- If the upstream window is unknown, the configured default duration is used.
- While blacklisted, calls return `TemporarilyBlacklisted { reset_in_ms }` immediately (cheap fast-fail).

## Caching middleware

`CacheMiddleware` adds per-capability, TTL-based caching on top of any connector. It supports:

- Positive caching of successful results per capability with independent TTLs and capacities
- Optional negative caching of permanent errors (e.g., Unsupported/NotFound) via a separate TTL
- Sensible defaults (e.g., very short for quotes, longer for fundamentals) that you can override

Configuration lives in `borsa_types::CacheConfig` and uses capability string keys (see `borsa_types::Capability::as_str()`). Setting a TTL to 0 disables caching for that capability.

### Example: Enable caching with overrides

```rust,ignore
use std::sync::Arc;
use borsa::Borsa;
use borsa_core::BorsaConnector;
use borsa_middleware::ConnectorBuilder;
use borsa_types::CacheConfig;
use borsa_mock::MockConnector;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Start from sensible defaults
    let mut cfg = CacheConfig::default();
    // Make quotes very fresh (2s) and cap the history cache size
    cfg.per_capability_ttl_ms.insert("quote".into(), 2_000);
    cfg.per_capability_max_entries.insert("history".into(), 200);
    // Disable negative caching globally
    cfg.default_negative_ttl_ms = 0;

    let raw: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());
    let wrapped = ConnectorBuilder::new(raw)
        .with_cache(&cfg)
        .build()?;

    let borsa = Borsa::builder().with_connector(wrapped).build()?;
    Ok(())
}
```

### Configuration reference (selected)

- `default_ttl_ms`: default TTL for all capabilities (0 disables)
- `per_capability_ttl_ms`: map of capability -> TTL override
- `default_max_entries`: default max entries per-capability
- `per_capability_max_entries`: map of capability -> capacity override
- `default_negative_ttl_ms`: default TTL for negative caching (0 disables)
- `per_capability_negative_ttl_ms`: map of capability -> negative TTL override

Common capability keys include: `quote`, `profile`, `history`, `search`, `option_chain`, `news`, fundamentals such as `income_statement`, `balance_sheet`, `cashflow`, holders such as `major_holders` and more. See `borsa_types::Capability` for the full list and defaults.

## Compose multiple middlewares

Use `ConnectorBuilder` to compose and order layers. The builder enforces a safe policy:

- Cache (outermost) → Blacklist → Quota → Raw

```rust,ignore
use std::sync::Arc;
use std::time::Duration;
use borsa::Borsa;
use borsa_core::BorsaConnector;
use borsa_middleware::ConnectorBuilder;
use borsa_types::{CacheConfig, QuotaConfig, QuotaConsumptionStrategy};
use borsa_mock::MockConnector;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raw: Arc<dyn BorsaConnector> = Arc::new(MockConnector::new());

    let mut cache = CacheConfig::default();
    cache.per_capability_ttl_ms.insert("quote".into(), 2_000);

    let quota = QuotaConfig {
        limit: 24,
        window: std::time::Duration::from_secs(24 * 60 * 60),
        strategy: QuotaConsumptionStrategy::EvenSpreadHourly,
    };

    let wrapped = ConnectorBuilder::new(raw)
        .with_cache(&cache)
        .with_blacklist(Duration::from_secs(300))
        .with_quota(&quota)
        .build()?;

    let borsa = Borsa::builder().with_connector(wrapped).build()?;
    Ok(())
}
```

## Notes

- Thread-safe and cheap to clone via `Arc`.
- Strategy `Weighted` exists in `borsa-types` but is not yet implemented by this wrapper.

## See also

- Main crate: `borsa`
- Core traits and types: `borsa-core`, `borsa-types`
