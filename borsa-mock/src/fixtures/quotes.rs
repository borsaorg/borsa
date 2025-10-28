use borsa_core::{Currency, Exchange, IsoCurrency, Money, Quote, Symbol};

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
        "KO" => Some(q("KO", "Coca-Cola", "60.00", "59.50", Exchange::NYSE)),
        "PEP" => Some(q("PEP", "PepsiCo", "170.00", "168.00", Exchange::NASDAQ)),
        _ => None,
    }
}

fn q(sym: &str, name: &str, px: &str, prev: &str, exch: Exchange) -> Quote {
    Quote {
        symbol: Symbol::new(sym).unwrap(),
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
