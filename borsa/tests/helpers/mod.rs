// Re-export helpers so tests can `use helpers::*;`
pub mod mock_connector;

pub use mock_connector::{MockConnector, candle, m_hist, m_quote, m_search};

use borsa_core::{AssetKind, Instrument};

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

/// Common symbol constants used across tests.
pub const AAPL: &str = "AAPL";
pub const MSFT: &str = "MSFT";
pub const TSLA: &str = "TSLA";
pub const X: &str = "X";
pub const GOOG: &str = "GOOG";
#[allow(dead_code)]
pub const BTC_USD: &str = "BTC-USD";
#[allow(dead_code)]
pub const ETH_USD: &str = "ETH-USD";

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
pub fn instrument(symbol: &str, kind: AssetKind) -> Instrument {
    Instrument::from_symbol(symbol, kind).expect("valid static test symbol")
}

/// Create a minimal Quote with only `symbol` and `price` populated.
pub fn quote_fixture(symbol: &str, price: &str) -> borsa_core::Quote {
    borsa_core::Quote {
        symbol: borsa_core::Symbol::new(symbol).expect("valid test symbol"),
        shortname: None,
        price: Some(usd(price)),
        previous_close: None,
        exchange: None,
        market_state: None,
        day_volume: None,
    }
}
