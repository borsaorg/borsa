# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2025-10-21

### Added

- New crate `borsa-types` for shared domain types and reports used across the
  workspace (configuration, connector keys, attribution, and report envelopes).

### Breaking Change

- Router download API redesigned for clarity and richer context:
  - `DownloadResponse.history: HashMap<Symbol, HistoryResponse>` replaced by
    `DownloadResponse.entries: Vec<DownloadEntry>` where each entry includes the
    `instrument` and its `history`.
  - Tests updated to assert over `entries` instead of map lookups.

### Changed

- Moved configuration (`BorsaConfig`, `BackoffConfig`), fetch/merge strategies,
  connector key, and attribution/span types into `borsa-types` and re-exported
  them from `borsa`/`borsa-core`.

### Removed

- Deleted `borsa/src/attrib.rs`; attribution types now live in `borsa-types`.

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
