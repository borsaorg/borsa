# borsa-types

Shared data transfer objects and configuration types for the Borsa ecosystem, built on top of the [`paft`](https://github.com/paft-rs/paft) primitives.

This crate centralizes:
- Error type: `borsa_types::BorsaError`
- Report envelopes: `InfoReport`, `SearchReport`, `DownloadReport`
- Attribution helpers: `Attribution`, `Span`
- Orchestrator configuration: `BorsaConfig`, `FetchStrategy`, `MergeStrategy`, `Resampling`, `BackoffConfig`

Most users only depend on `borsa` or `borsa-core`, which re-export these types for convenience. Depend on `borsa-types` directly if you:
- Serialize/deserialize reports or errors directly
- Share types across binaries/services without pulling router or connector traits
- Need to construct configuration types without `borsa`

## Install

```toml
[dependencies]
borsa-types = "0.3.0"
```

## Documentation

- API docs: https://docs.rs/borsa-types
- Related crates: `borsa` (router/orchestrator), `borsa-core` (traits + utilities)
