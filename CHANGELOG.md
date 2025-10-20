# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2] - Unreleased

### Added

- Optional tracing feature across the workspace:
  - `borsa`, `borsa-core`, and `borsa-yfinance` expose a `tracing` feature flag
  - Router entry points and core orchestration in `borsa` emit spans when enabled
  - `borsa-yfinance` instruments all public provider endpoints (quotes, history, search, profile, fundamentals, options, analysis, holders, ESG, news, streaming)
- New example `examples/examples/00_tracing.rs` showing how to initialize `tracing_subscriber` and view spans
- Documentation updates in `README.md`, `borsa/README.md`, and `borsa-yfinance/README.md` describing observability usage and run commands

### Fixed

- router: stream startup now fails if any connector fails to initialize, aborting
  spawned tasks and returning a consolidated error instead of partially starting.

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
