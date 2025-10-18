use borsa_core::{Candle, Currency, HistoryResponse, IsoCurrency, Money};

pub fn by_symbol(s: &str) -> Option<HistoryResponse> {
    match s {
        "AAPL" => Some(build(vec![
            ("2023-01-02", "140", "142", "139", "141", 10_000_000),
            ("2023-01-03", "141", "143", "140", "142", 11_000_000),
        ])),
        "MSFT" => Some(build(vec![
            ("2023-01-02", "240", "245", "238", "244", 9_000_000),
            ("2023-01-03", "244", "246", "243", "245", 9_500_000),
        ])),
        "GOOG" => Some(build(vec![
            ("2023-01-02", "100", "110", "95", "105", 5_000_000),
            ("2023-01-03", "105", "112", "102", "110", 5_500_000),
        ])),
        "TSLA" => Some(build(vec![
            ("2023-01-02", "300", "310", "295", "305", 8_000_000),
            ("2023-01-03", "305", "315", "300", "312", 8_500_000),
        ])),
        _ => None,
    }
}

fn usd(s: &str) -> Money {
    Money::from_canonical_str(s, Currency::Iso(IsoCurrency::USD)).unwrap()
}

fn build(rows: Vec<(&str, &str, &str, &str, &str, i64)>) -> HistoryResponse {
    let candles = rows
        .into_iter()
        .map(|(date, o, h, l, c, v)| Candle {
            ts: chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc(),
            open: usd(o),
            high: usd(h),
            low: usd(l),
            close: usd(c),
            close_unadj: None,
            volume: Some(v as u64),
        })
        .collect();
    HistoryResponse {
        candles,
        actions: vec![],
        adjusted: false,
        meta: None,
    }
}
