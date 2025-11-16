#![cfg(feature = "test-adapters")]

use std::sync::Arc;

use borsa_core::{
    AssetKind, BorsaConnector, Currency, Decimal, Instrument, Money, Period, RecommendationAction,
    connector::{
        AnalystPriceTargetProvider, RecommendationsProvider, RecommendationsSummaryProvider,
        UpgradesDowngradesProvider,
    },
};
use borsa_yfinance::{YfConnector, adapter};
use chrono::TimeZone;

struct Combo {
    a: Arc<dyn adapter::YfAnalysis>,
}
impl adapter::CloneArcAdapters for Combo {
    fn clone_arc_analysis(&self) -> Arc<dyn adapter::YfAnalysis> {
        self.a.clone()
    }
}

#[tokio::test]
async fn analysis_uses_injected_adapter_and_maps() {
    // Fake analysis adapter returns deterministic data; assert symbol flows through.
    let ana = <dyn adapter::YfAnalysis>::from_fns(
        |sym| {
            assert_eq!(sym, "AAPL");
            Ok(vec![yfinance_rs::analysis::RecommendationRow {
                period: "2024-08".parse::<Period>().unwrap(),
                strong_buy: Some(5),
                buy: Some(10),
                hold: Some(7),
                sell: Some(1),
                strong_sell: Some(0),
            }])
        },
        |sym| {
            assert_eq!(sym, "AAPL");
            Ok(yfinance_rs::analysis::RecommendationSummary {
                latest_period: Some("2024-08".parse::<Period>().unwrap()),
                strong_buy: Some(5),
                buy: Some(10),
                hold: Some(7),
                sell: Some(1),
                strong_sell: Some(0),
                mean: Some(dec("1.9")),
                mean_rating_text: None,
            })
        },
        |sym| {
            assert_eq!(sym, "AAPL");
            Ok(vec![yfinance_rs::analysis::UpgradeDowngradeRow {
                ts: chrono::Utc.timestamp_opt(1_720_000_000, 0).unwrap(),
                firm: Some("ABC".into()),
                from_grade: Some(borsa_core::RecommendationGrade::Hold),
                to_grade: Some(borsa_core::RecommendationGrade::Buy),
                action: Some(RecommendationAction::Upgrade),
            }])
        },
        |sym| {
            assert_eq!(sym, "AAPL");
            Ok(yfinance_rs::analysis::PriceTarget {
                mean: Some(
                    Money::from_canonical_str("210.0", Currency::Iso(borsa_core::IsoCurrency::USD))
                        .unwrap(),
                ),
                high: Some(
                    Money::from_canonical_str("250.0", Currency::Iso(borsa_core::IsoCurrency::USD))
                        .unwrap(),
                ),
                low: Some(
                    Money::from_canonical_str("180.0", Currency::Iso(borsa_core::IsoCurrency::USD))
                        .unwrap(),
                ),
                number_of_analysts: Some(42),
            })
        },
    );

    // Everything else via defaults
    let yf = YfConnector::from_adapter(&Combo { a: ana });
    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid test instrument");

    let recs = yf.recommendations(&inst).await.unwrap();
    assert_eq!(recs.len(), 1);
    assert_eq!(recs[0].period, "2024-08".parse::<Period>().unwrap());

    let sum = yf.recommendations_summary(&inst).await.unwrap();
    assert_eq!(
        sum.latest_period,
        Some("2024-08".parse::<Period>().unwrap())
    );
    assert_eq!(sum.mean, Some(dec("1.9")));

    let uds = yf.upgrades_downgrades(&inst).await.unwrap();
    assert_eq!(uds.len(), 1);
    assert_eq!(uds[0].firm.as_deref(), Some("ABC"));

    let pt = yf.analyst_price_target(&inst).await.unwrap();
    assert_eq!(
        pt.high,
        Some(
            Money::from_canonical_str("250.0", Currency::Iso(borsa_core::IsoCurrency::USD))
                .unwrap()
        )
    );
    assert_eq!(pt.number_of_analysts, Some(42));
}

#[test]
fn yf_connector_advertises_analysis_capability() {
    let yf = YfConnector::new_default();
    assert!(yf.as_recommendations_provider().is_some());
    assert!(yf.as_recommendations_summary_provider().is_some());
    assert!(yf.as_upgrades_downgrades_provider().is_some());
    assert!(yf.as_analyst_price_target_provider().is_some());
}

#[test]
fn analysis_injection_periods_and_actions() {
    let p: Period = "2024-08".parse().unwrap();
    let _ = p; // ensure parsing works across paft versions without asserting specific variant
    let a: RecommendationAction = "up".parse().unwrap();
    assert_eq!(a.code(), "UPGRADE");
}

fn dec(input: &str) -> Decimal {
    input.parse().expect("valid decimal literal")
}
