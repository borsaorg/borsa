use borsa_core::{AssetKind, Currency, Exchange, Instrument, IsoCurrency, Money, Quote};

pub fn by_symbol(s: &str) -> Option<Quote> {
    match s {
        "AAPL" => Some(q(
            "AAPL",
            "Apple Inc.",
            "190.00",
            "188.00",
            Exchange::NASDAQ,
        )),
        "MSFT" => Some(q(
            "MSFT",
            "Microsoft Corp",
            "420.00",
            "418.00",
            Exchange::NASDAQ,
        )),
        "NVDA" => Some(q(
            "NVDA",
            "NVIDIA Corp",
            "1000.00",
            "990.00",
            Exchange::NASDAQ,
        )),
        "GOOGL" => Some(q(
            "GOOGL",
            "Alphabet Inc. Class A",
            "150.00",
            "148.00",
            Exchange::NASDAQ,
        )),
        "KO" => Some(q("KO", "Coca-Cola", "60.00", "59.50", Exchange::NYSE)),
        "PEP" => Some(q("PEP", "PepsiCo", "170.00", "168.00", Exchange::NASDAQ)),
        "BTC-USD" => Some(Quote {
            instrument: Instrument::from_symbol("BTC-USD", AssetKind::Crypto).unwrap(),
            shortname: Some("Bitcoin USD".to_string()),
            price: Some(
                Money::from_canonical_str("65000.00", Currency::Iso(IsoCurrency::USD)).unwrap(),
            ),
            previous_close: Some(
                Money::from_canonical_str("64000.00", Currency::Iso(IsoCurrency::USD)).unwrap(),
            ),
            exchange: None,
            market_state: None,
            day_volume: None,
        }),
        _ => None,
    }
}

fn q(sym: &str, name: &str, px: &str, prev: &str, exch: Exchange) -> Quote {
    Quote {
        instrument: Instrument::from_symbol(sym, AssetKind::Equity).unwrap(),
        shortname: Some(name.to_string()),
        price: Some(Money::from_canonical_str(px, Currency::Iso(IsoCurrency::USD)).unwrap()),
        previous_close: Some(
            Money::from_canonical_str(prev, Currency::Iso(IsoCurrency::USD)).unwrap(),
        ),
        exchange: Some(exch),
        market_state: None,
        day_volume: None,
    }
}
