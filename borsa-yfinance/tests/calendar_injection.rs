#![cfg(feature = "test-adapters")]

// In `borsa-yfinance/tests/calendar_injection.rs`
use std::sync::Arc;

use async_trait::async_trait;
use borsa_core::{AssetKind, Instrument, connector::CalendarProvider};
use borsa_yfinance::{YfConnector, adapter};
use chrono::TimeZone;
use yfinance_rs as yf;

struct StubFundamentals;
#[async_trait]
impl adapter::YfFundamentals for StubFundamentals {
    async fn earnings(
        &self,
        _symbol: &str,
    ) -> Result<yf::fundamentals::Earnings, borsa_core::BorsaError> {
        unreachable!()
    }
    async fn calendar(
        &self,
        _symbol: &str,
    ) -> Result<yf::fundamentals::Calendar, borsa_core::BorsaError> {
        Ok(yf::fundamentals::Calendar {
            earnings_dates: vec![
                chrono::Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
                chrono::Utc.timestamp_opt(1_700_086_400, 0).unwrap(),
            ],
            ex_dividend_date: Some(chrono::Utc.timestamp_opt(1_700_043_200, 0).unwrap()),
            dividend_payment_date: Some(chrono::Utc.timestamp_opt(1_700_086_400, 0).unwrap()),
        })
    }
}

struct Combo {
    f: Arc<dyn adapter::YfFundamentals>,
}
impl adapter::CloneArcAdapters for Combo {
    fn clone_arc_fundamentals(&self) -> Arc<dyn adapter::YfFundamentals> {
        self.f.clone()
    }
}

#[tokio::test]
async fn calendar_injection_maps_correctly() {
    let yf = YfConnector::from_adapter(&Combo {
        f: Arc::new(StubFundamentals),
    });
    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid test instrument");

    let cal = yf.calendar(&inst).await.unwrap();
    assert!(!cal.earnings_dates.is_empty());
    assert!(cal.ex_dividend_date.is_some());
}
