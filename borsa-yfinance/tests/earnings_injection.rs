#![cfg(feature = "test-adapters")]

use std::sync::Arc;

use borsa_core::{AssetKind, Currency, Instrument, Money, Period, connector::EarningsProvider};
use borsa_yfinance::{YfConnector, adapter};

struct Combo {
    f: Arc<dyn adapter::YfFundamentals>,
}
impl adapter::CloneArcAdapters for Combo {
    fn clone_arc_fundamentals(&self) -> Arc<dyn adapter::YfFundamentals> {
        self.f.clone()
    }
}

#[tokio::test]
async fn earnings_uses_injected_adapter_and_maps() {
    let fun = <dyn adapter::YfFundamentals>::from_fns(
        |_sym, _q| {
            Err(borsa_core::BorsaError::unsupported(
                "fundamentals/income_statement",
            ))
        },
        |_sym, _q| {
            Err(borsa_core::BorsaError::unsupported(
                "fundamentals/balance_sheet",
            ))
        },
        |_sym, _q| Err(borsa_core::BorsaError::unsupported("fundamentals/cashflow")),
        |_sym| Err(borsa_core::BorsaError::unsupported("fundamentals/calendar")),
        |symbol| {
            assert_eq!(symbol, "MSFT");

            Ok(yfinance_rs::fundamentals::Earnings {
                yearly: vec![yfinance_rs::fundamentals::EarningsYear {
                    year: 2023,
                    revenue: Some(
                        Money::from_canonical_str(
                            "211000",
                            Currency::Iso(borsa_core::IsoCurrency::USD),
                        )
                        .unwrap(),
                    ),
                    earnings: Some(
                        Money::from_canonical_str(
                            "72000",
                            Currency::Iso(borsa_core::IsoCurrency::USD),
                        )
                        .unwrap(),
                    ),
                }],
                quarterly: vec![yfinance_rs::fundamentals::EarningsQuarter {
                    period: "2024Q1".parse::<Period>().unwrap(),
                    revenue: Some(
                        Money::from_canonical_str(
                            "61000",
                            Currency::Iso(borsa_core::IsoCurrency::USD),
                        )
                        .unwrap(),
                    ),
                    earnings: Some(
                        Money::from_canonical_str(
                            "21000",
                            Currency::Iso(borsa_core::IsoCurrency::USD),
                        )
                        .unwrap(),
                    ),
                }],
                quarterly_eps: vec![yfinance_rs::fundamentals::EarningsQuarterEps {
                    period: "2024Q1".parse::<Period>().unwrap(),
                    actual: Some(
                        Money::from_canonical_str(
                            "2.99",
                            Currency::Iso(borsa_core::IsoCurrency::USD),
                        )
                        .unwrap(),
                    ),
                    estimate: Some(
                        Money::from_canonical_str(
                            "2.70",
                            Currency::Iso(borsa_core::IsoCurrency::USD),
                        )
                        .unwrap(),
                    ),
                }],
            })
        },
    );

    let yf = YfConnector::from_adapter(&Combo { f: fun });

    let inst = Instrument::from_symbol("MSFT", AssetKind::Equity).expect("valid test instrument");
    let out = yf.earnings(&inst).await.unwrap();

    assert_eq!(out.yearly.len(), 1);
    assert_eq!(out.yearly[0].year, 2023);
    let p = "2024Q1".parse::<Period>().unwrap();
    assert!(out.quarterly.iter().any(|q| q.period == p));
    assert!(
        out.quarterly_eps
            .iter()
            .any(|e| e.actual.as_ref().map(|m| m.amount().to_string()) == Some("2.99".into()))
    );
}

#[test]
fn earnings_injection_periods() {
    let p: Period = "2024Q1".parse().unwrap();
    assert!(p.is_quarterly() || p.is_annual());
}
