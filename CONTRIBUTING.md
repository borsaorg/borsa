# Contributing to borsa

Thank you for your interest in contributing! This workspace hosts multiple crates that compose the borsa ecosystem: a connector trait and core types, a high-level router, and provider connectors. We welcome fixes, features, docs, tests, and new connectors.

## Code of Conduct

Please review and follow our [Code of Conduct](https://github.com/borsaorg/borsa/blob/main/CODE_OF_CONDUCT.md)

## Development setup

- Rust toolchain: stable (edition 2024)
- Run tests: `just test`
- Lint: `just lint`
- Format: `just fmt`

## Repository layout

- `borsa-core`: core types, errors, and the `BorsaConnector` trait
- `borsa`: the high-level router/orchestrator
- `borsa-yfinance`: Yahoo Finance connector
- `examples/`: runnable example programs

## What to work on

- Good first issues: small doc fixes, examples, or tests
- Features: extending router capabilities or adding data domains
- Providers: implement a new connector crate for a data vendor

## Workflow

1. Fork the repo and create a feature branch
2. Make focused commits with clear messages
3. Ensure `cargo test --workspace` passes and no clippy warnings remain
4. Update docs and examples as needed
5. Open a PR describing the change, motivation, and testing

## Implementing a new connector

New connectors live as their own crate at the workspace root (e.g., `borsa-someprovider/`). A connector implements the `BorsaConnector` trait from `borsa-core` and exposes a small, ergonomic constructor.

Key steps:

1. Create a crate: `cargo new borsa-someprovider --lib`
2. Add dependencies (in crate `Cargo.toml`):
   - `borsa-core = { workspace = true }`
   - Other client libs for the vendor API
3. Implement the connector in `src/lib.rs` using capability accessors:
   - `fn name(&self) -> &'static str` returns a stable identifier (e.g., `"borsa-someprovider"`)
   - `fn vendor(&self) -> &'static str` returns a human-friendly vendor name
   - `fn supports_kind(&self, kind: AssetKind) -> bool` declares supported asset kinds (e.g., equities, crypto)
   - Advertise capabilities via `as_*_provider` accessors on `BorsaConnector` (e.g., `as_quote_provider`, `as_history_provider`, `as_search_provider`, ...)
   - Implement async methods on the corresponding role traits you support (e.g., `QuoteProvider::quote`, `HistoryProvider::history`, `SearchProvider::search`, ...)
4. For history, implement `HistoryProvider` and declare native intervals via `fn supported_history_intervals(&self, kind: AssetKind) -> &'static [Interval]` on the provider implementation (not on the connector trait).
5. Map vendor-specific models to `borsa-core` types
6. Add tests (unit + integration) validating:
   - Interval mapping and errors for unsupported intervals
   - Data kind support (e.g., equities, crypto, forex)
   - Error handling: `NotFound` vs `Unsupported` vs `Other`
7. Document crate usage in `README.md` and note any required environment variables (e.g., API keys)

Reference implementations:

 - `borsa-yfinance/src/lib.rs`
 - Trait: `borsa-core/src/connector.rs`

## Data model and routing

- Prefer to return `BorsaError::NotFound` for symbols or datasets that the provider does not have, so the router can fall back to lower-priority connectors.
- Use `BorsaError::unsupported("feature")` for capabilities the provider cannot serve.
- History intervals: return only the exact native intervals in `supported_history_intervals`; the router may resample intraday data to the requested interval.
- For overlapping history from multiple connectors, the router can prioritize adjusted data (`prefer_adjusted_history(true)`) and resample to daily/weekly.

## Testing guidance

- Keep tests deterministic; use recorded fixtures or mock connectors where possible
- Avoid hitting external APIs in CI; if unavoidable, guard behind opt-in flags
- Exercise corner cases: empty series, missing fields, timezone metadata, actions alignment

## Commit conventions

- Use present tense: "Add X", "Fix Y"
- Keep the first line under ~72 chars; include additional context in the body
- Reference issues when applicable

## Security and disclosures

If you discover a vulnerability, please email the maintainers privately at security@borsa.rs rather than opening a public issue.

## License

By contributing, you agree that your contributions are licensed under the MIT License (see `LICENSE`).
