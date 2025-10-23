# borsa-mock

A mock connector for the Borsa ecosystem used in tests and examples. It provides deterministic fixture data across capabilities (quotes, history, fundamentals, options, analysis, news, search, profile) to enable fast, reproducible scenarios without external network calls.

## Usage

Add the crate as a dev-dependency and enable it in tests or examples:

```toml
[dev-dependencies]
borsa-mock = "0.2.0"
```

Then wire it as a provider when constructing `borsa::Borsa` in your tests.

## License

MIT
