// Re-export helpers so tests can `use helpers::*;`
pub mod mock_connector;

pub use mock_connector::{MockConnector, StreamStep, candle, m_hist, m_quote, m_search};

use borsa_core::{AssetKind, Instrument, Symbol};
use std::sync::LazyLock;

#[doc(hidden)]
pub const INTERVALS: &[borsa_core::Interval] = &[
    borsa_core::Interval::I1m,
    borsa_core::Interval::I2m,
    borsa_core::Interval::I15m,
    borsa_core::Interval::I30m,
    borsa_core::Interval::I90m,
    borsa_core::Interval::D1,
    borsa_core::Interval::W1,
];

// ---------- Lightweight fixtures and helpers for tests ----------

pub static AAPL: LazyLock<Symbol> = LazyLock::new(|| Symbol::new("AAPL").expect("valid symbol"));
pub static MSFT: LazyLock<Symbol> = LazyLock::new(|| Symbol::new("MSFT").expect("valid symbol"));
pub static TSLA: LazyLock<Symbol> = LazyLock::new(|| Symbol::new("TSLA").expect("valid symbol"));
pub static X: LazyLock<Symbol> = LazyLock::new(|| Symbol::new("X").expect("valid symbol"));
pub static GOOG: LazyLock<Symbol> = LazyLock::new(|| Symbol::new("GOOG").expect("valid symbol"));
pub static BTC_USD: LazyLock<Symbol> =
    LazyLock::new(|| Symbol::new("BTC-USD").expect("valid symbol"));
pub static ETH_USD: LazyLock<Symbol> =
    LazyLock::new(|| Symbol::new("ETH-USD").expect("valid symbol"));

/// Construct a UTC `DateTime` from components for readability in tests.
pub const fn dt(
    y: i32,
    m: u32,
    d: u32,
    hh: u32,
    mm: u32,
    ss: u32,
) -> chrono::DateTime<chrono::Utc> {
    let date = chrono::NaiveDate::from_ymd_opt(y, m, d).expect("invalid date");
    let naive = date
        .and_hms_opt(hh, mm, ss)
        .expect("invalid time components");
    chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(naive, chrono::Utc)
}

/// Convenience to derive a UNIX timestamp (seconds) from date components.
pub const fn ts(y: i32, m: u32, d: u32, hh: u32, mm: u32, ss: u32) -> i64 {
    dt(y, m, d, hh, mm, ss).timestamp()
}

/// Build a USD Money amount without unwrap noise in tests.
pub fn usd(amount: &str) -> borsa_core::Money {
    borsa_core::Money::from_canonical_str(
        amount,
        borsa_core::Currency::Iso(borsa_core::IsoCurrency::USD),
    )
    .unwrap()
}

/// Construct an `Instrument` for test usage with infallible expectations.
pub fn instrument(symbol: &Symbol, kind: AssetKind) -> Instrument {
    Instrument::from_symbol(symbol, kind).expect("valid static test symbol")
}

/// Create a minimal Quote with only `symbol` and `price` populated.
pub fn quote_fixture(symbol: &Symbol, price: &str) -> borsa_core::Quote {
    borsa_core::Quote {
        symbol: symbol.clone(),
        shortname: None,
        price: Some(usd(price)),
        previous_close: None,
        exchange: None,
        market_state: None,
        day_volume: None,
    }
}
