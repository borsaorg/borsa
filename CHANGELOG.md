# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2025-10-18

### Added

- Automated release changelog generation in GitHub Actions using `taiki-e/create-gh-release-action`.

### Fixed

- borsa-yfinance: Expose ISIN capability via `IsinProvider` and `as_isin_provider` so `borsa::Borsa::isin` routes correctly.

### Changed

- Bump workspace package version to `0.1.1` and align member crate versions.
- Update README dependency snippets to reference `0.1.1`.
- borsa-mock: Add crate metadata (`readme`, `keywords`, `categories`).
- borsa-core: Re-export dataframe traits from `paft::core::dataframe` (replacing `paft-utils` path).
- workspace: Move `proptest` and `loom` into workspace dev-dependencies for consistency.

### Removed

- Remove crate-level `Cargo.lock` files in favor of the workspace `Cargo.lock`.
- borsa-core: Remove optional dependency on `paft-utils`.

## [0.1.0] - 2025-10-18

### Added

- Initial release of the `borsa` ecosystem.
- Core traits and types in `borsa-core` for building financial data connectors.
- High-level orchestrator `borsa` with provider routing, merging, and resampling.
- Yahoo Finance connector `borsa-yfinance` with comprehensive data support.
- Support for quotes, historical data, fundamentals, options, analysis, news, and streaming.
