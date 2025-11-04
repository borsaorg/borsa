# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2025-11-XX

This release focuses heavily on production reliability and developer experience. The streaming system has been completely rewritten to handle network failures, provider outages, and edge cases gracefully, with fixes for memory leaks and stale data. The new middleware system is the flagship feature, enabling automatic quota management, intelligent rate limiting, provider blacklisting, and caching. Type safety improvements throughout (particularly the Capability enum and proper Symbol types) reduce runtime errors. Extensive property-based testing and the new dynamic mock connector significantly improve testability for applications built on borsa.

### Added

- **Middleware System**: Introduced comprehensive middleware infrastructure with quota management, provider blacklisting, and caching support
  - Quota enforcement with `EvenSpreadHourly` strategy to prevent rate limit violations
  - Automatic provider blacklisting on rate limit errors with configurable timeouts
  - Type-safe middleware validation and ordering
  - Proc-macro based middleware delegation to reduce boilerplate
- **Streaming Improvements**:
  - Per-symbol monotonic timestamp enforcement (enabled by default)
  - Manual stream control via `push_update` for external updates in mock connector
  - Concurrent multi-session streaming architecture for improved reliability
- **Enhanced Info API**: Added `volume` field to `FastInfo` and additional `Info` fields
- **Search Enhancements**: Yahoo Finance search now supports `lang` and `region` parameters
- **Mock Connector**: New dynamic mock connector/controller for testing
- **Error Handling**: Added retry classification and structured error handling helpers
- **Type Safety**: `OptionContract` now re-exported from `borsa_core` for convenience
- **Builder Enhancement**: New method to create unconfigured `YfConnectorBuilder`

### Breaking Changes

- **Streaming Architecture**: Complete rewrite of streaming implementation with policy-based per-symbol routing and improved failover
- **Capability System**: Replaced string-based capability labels with type-safe `Capability` enum
- **Symbol Types**: Now using `paft::domain::Symbol` type instead of raw `String` for better type safety

### Changed

- Improved provider failover logic to maintain subscription coverage during transitions
- Better error normalization across providers with structured connector errors
- Quota window boundaries now align to prevent drift in rate limiting
- Streaming now properly handles wildcard subscriptions merged with explicit symbol groups
- Middleware ordering ensures unsupported operations skip quota checks

### Fixed

- **Streaming Reliability**:
  - Prevented delayed failover by tracking connection states and allowing concurrent starts
  - Fixed memory leaks via per-session gates with TTL
  - Resolved stale ordering issues after reconnection by resetting monotonic gates
  - Prevented dropped updates during provider failover
- **Routing**: Mixed-currency history requests now use priority-based fault attribution
- **Search**: Provider errors now included in warnings for partial results
- **Builder**: Quota fields now properly preserved across chained setters
- **Blacklist**: Now mutes providers on `RateLimitExceeded` errors with appropriate timeouts

### Dependencies

- Updated `paft` to 0.7.2
- Updated `yfinance-rs` to 0.7.2
- Added `sync` feature to `tokio` dependency

### Documentation

- Documented middleware ordering conventions in builder
- Added comprehensive middleware README
- Rewrote fundamentals deep dive example
- Reorganized examples to `borsa/examples` directory

### Removed

- **ESG Provider**: Disabled on borsa-yfinance due to missing Yahoo Finance API endpoint


## [0.2.0] - 2025-10-21

### Added

- New crate `borsa-types` for shared domain types and reports used across the
  workspace (configuration, connector keys, attribution, and report envelopes).
- Unified routing policy for provider and exchange ordering:
  - `borsa-types::routing_policy::{RoutingPolicy, RoutingPolicyBuilder}` with
    composable global/kind/symbol/exchange rules and an optional `strict` flag.
  - Exchange preferences are used for search de-duplication (Symbol > Kind > Global).
- `BorsaConnector::key()` helper for typed connector keys when building policies.
- `BorsaError::StrictSymbolsRejected` to surface strict policy exclusions in streaming.
- Full serde support for `BorsaConfig` enums and `ConnectorKey` to enable config-as-data.

### Breaking Change

- Router download API redesigned for clarity and richer context:
  - `DownloadResponse.history: HashMap<Symbol, HistoryResponse>` replaced by
    `DownloadResponse.entries: Vec<DownloadEntry>` where each entry includes the
    `instrument` and its `history`.
  - Tests updated to assert over `entries` instead of map lookups.
- Unified error moved to `borsa-types::BorsaError` and now derives `Serialize`/`Deserialize`.
  - Capability fields on `BorsaError::{Unsupported,ProviderTimeout,RequestTimeout,AllProvidersTimedOut}`
     are now `String` instead of `&'static str`.
  - Report envelopes (`InfoReport`, `SearchReport`, `DownloadReport`) change `warnings`
     from `Vec<String>` to `Vec<BorsaError>` and propagate structured errors instead of strings.
- `borsa-core::error` module removed; import `BorsaError` via `borsa_core::types::BorsaError`.
- Builder APIs `prefer_for_kind(...)` and `prefer_symbol(...)` were removed and
  replaced by a unified `routing_policy(...)` configuration built with
  `RoutingPolicyBuilder`. Update builder calls and tests accordingly.
- borsa-yfinance: `YfConnector::KEY` constant is replaced by `BorsaConnector::key()`
  or `YfConnector::key_static()` for constructing typed keys.

### Changed

- Moved configuration (`BorsaConfig`, `BackoffConfig`), fetch/merge strategies,
  connector key, and attribution/span types into `borsa-types` and re-exported
  them from `borsa`/`borsa-core`.
- Routers now preserve connector-tagged errors in report `warnings`.
- router/history: attribution is now derived from the merged timeline using
  first-wins per timestamp; spans group contiguous provider segments regardless
  of cadence gaps for clearer provenance.
- examples: switch fixed epoch timestamps to recent 00:00 UTC dates for
  readability and stability across runs; compute daily candle counts explicitly.
- examples: implement `supports_kind` on mock connectors to match the current
  connector trait.
- quote routing: enforce instrument exchange on successful quotes; exchange
  mismatch is treated as `NotFound` to enable priority fallback/latency racing.
- search: de-duplicate cross-provider results by symbol using configured
  exchange preferences (Symbol > Kind > Global) while preserving traversal order.
- streaming: assign symbols per provider subset based on routing policy and drop
  updates for unassigned symbols; strict rules can proactively reject symbols.
- `BorsaConfig` now carries a unified `routing_policy` and is fully
  `Serialize`/`Deserialize` for external configuration.

### Fixed

- router/history: avoid fragmented or misleading attribution when providers have
  gaps or differing cadences by building runs on the global merged sequence.

### Removed

- Deleted `borsa/src/attrib.rs`; attribution types now live in `borsa-types`.
- Deleted `borsa-core/src/error.rs`; error type lives in `borsa-types`.

### Dependencies

- Bump `paft` to `v0.6.0`.
- Bump `yfinance-rs` to `v0.6.0`.

## [0.1.2] - 2025-10-20

### Added

- Optional tracing feature across the workspace:
  - `borsa`, `borsa-core`, and `borsa-yfinance` expose a `tracing` feature flag
  - Router entry points and core orchestration in `borsa` emit spans when enabled
  - `borsa-yfinance` instruments all public provider endpoints (quotes, history, search, profile, fundamentals, options, analysis, holders, ESG, news, streaming)
- New example `examples/examples/00_tracing.rs` showing how to initialize `tracing_subscriber` and view spans

### Documentation

- Updated `README.md`, `borsa/README.md`, and `borsa-yfinance/README.md` describing observability usage and run commands

### Fixed

- router: stream startup now fails if any connector fails to initialize, aborting
  spawned tasks and returning a consolidated error instead of partially starting.
- router/info: suppress warnings for optional data in info report by filtering
  out `Unsupported` and `NotFound` errors; only actionable errors are retained.
- router/history: validate per-provider candle currencies; error on inconsistent
   series; ignore providers with no currency data when determining majority currency.
- router/search: return `Unsupported` when no providers support the requested
   capability; ignore non-attempted connectors in result merging and error
   aggregation.
- core: correct `merge_history` adjusted flag semantics to gate on the first
  contributing response and require all contributing responses to be adjusted.
- core: ensure `merge_history.meta` falls back to the first available meta when
  no candles contribute to the merged series, preserving timezone context.
- borsa-mock: replace blocking `std::thread::sleep` with non-blocking
   `tokio::time::sleep` in TIMEOUT simulation to avoid blocking the async runtime.

### Dependencies

- Bump `paft` to `v0.5.2`.
- Bump `yfinance-rs` to `v0.5.2`.
- Bump `syn` to `v2.0.107`.

## [0.1.1] - 2025-10-19

### Fixed

- borsa-yfinance: Expose ISIN capability via `IsinProvider` and `as_isin_provider` so `borsa::Borsa::isin` routes correctly.

### Changed

- borsa-core: Re-export dataframe traits from `paft::core::dataframe` (replacing `paft-utils` path).

## [0.1.0] - 2025-10-18

### Added

- Initial release of the `borsa` ecosystem.
- Core traits and types in `borsa-core` for building financial data connectors.
- High-level orchestrator `borsa` with provider routing, merging, and resampling.
- Yahoo Finance connector `borsa-yfinance` with comprehensive data support.
- Support for quotes, historical data, fundamentals, options, analysis, news, and streaming.
