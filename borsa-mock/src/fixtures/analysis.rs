use borsa_core::{RecommendationRow, RecommendationSummary, UpgradeDowngradeRow, PriceTarget, Money, Currency, IsoCurrency};

pub const fn recommendations_by_symbol(_s: &str) -> Vec<RecommendationRow> {
    vec![]
}

pub fn recommendations_summary_by_symbol(_s: &str) -> Option<RecommendationSummary> {
    Some(RecommendationSummary {
        latest_period: Some("2024-08".parse().unwrap()),
        strong_buy: Some(5),
        buy: Some(10),
        hold: Some(8),
        sell: Some(2),
        strong_sell: Some(1),
        mean: Some(2.0),
        mean_rating_text: Some("Buy".to_string()),
    })
}

pub const fn upgrades_downgrades_by_symbol(_s: &str) -> Vec<UpgradeDowngradeRow> {
    vec![]
}

pub fn price_target_by_symbol(_s: &str) -> PriceTarget {
    PriceTarget {
        low: Some(usd("150.0")),
        mean: Some(usd("200.0")),
        high: Some(usd("250.0")),
        number_of_analysts: Some(20),
    }
}

fn usd(s: &str) -> Money {
    Money::from_canonical_str(s, Currency::Iso(IsoCurrency::USD)).unwrap()
}
