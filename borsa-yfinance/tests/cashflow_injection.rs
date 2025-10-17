#![cfg(feature = "test-adapters")]

// In `borsa-yfinance/tests/cashflow_injection.rs`
use std::sync::Arc;

use async_trait::async_trait;
use borsa_core::{
    AssetKind, BorsaError, Currency, Instrument, Money, Period, connector::CashflowProvider,
};
use borsa_yfinance::{YfConnector, adapter};
use chrono::TimeZone;
use yfinance_rs as yf;

struct StubFundamentals;
#[async_trait]
impl adapter::YfFundamentals for StubFundamentals {
    async fn earnings(&self, _symbol: &str) -> Result<yf::fundamentals::Earnings, BorsaError> {
        Err(BorsaError::unsupported("earnings"))
    }
    async fn income_statement(
        &self,
        _symbol: &str,
        _quarterly: bool,
    ) -> Result<Vec<yf::fundamentals::IncomeStatementRow>, BorsaError> {
        Err(BorsaError::unsupported("income_statement"))
    }
    async fn balance_sheet(
        &self,
        _symbol: &str,
        _quarterly: bool,
    ) -> Result<Vec<yf::fundamentals::BalanceSheetRow>, BorsaError> {
        Err(BorsaError::unsupported("balance_sheet"))
    }
    async fn cashflow(
        &self,
        symbol: &str,
        quarterly: bool,
    ) -> Result<Vec<yf::fundamentals::CashflowRow>, BorsaError> {
        assert_eq!(symbol, "GOOG");
        assert!(quarterly, "expected quarterly=true");
        Ok(vec![
            yf::fundamentals::CashflowRow {
                period: Period::Date(
                    chrono::Utc
                        .timestamp_opt(1_700_000_000, 0)
                        .unwrap()
                        .date_naive(),
                ),
                operating_cashflow: Some(
                    Money::from_canonical_str(
                        "99000000000",
                        Currency::Iso(borsa_core::IsoCurrency::USD),
                    )
                    .unwrap(),
                ),
                capital_expenditures: Some(
                    Money::from_canonical_str(
                        "-31000000000",
                        Currency::Iso(borsa_core::IsoCurrency::USD),
                    )
                    .unwrap(),
                ),
                free_cash_flow: Some(
                    Money::from_canonical_str(
                        "68000000000",
                        Currency::Iso(borsa_core::IsoCurrency::USD),
                    )
                    .unwrap(),
                ),
                net_income: Some(
                    Money::from_canonical_str(
                        "60000000000",
                        Currency::Iso(borsa_core::IsoCurrency::USD),
                    )
                    .unwrap(),
                ),
            },
            yf::fundamentals::CashflowRow {
                period: Period::Date(
                    chrono::Utc
                        .timestamp_opt(1_668_000_000, 0)
                        .unwrap()
                        .date_naive(),
                ),
                operating_cashflow: Some(
                    Money::from_canonical_str(
                        "85000000000",
                        Currency::Iso(borsa_core::IsoCurrency::USD),
                    )
                    .unwrap(),
                ),
                capital_expenditures: Some(
                    Money::from_canonical_str(
                        "-29000000000",
                        Currency::Iso(borsa_core::IsoCurrency::USD),
                    )
                    .unwrap(),
                ),
                free_cash_flow: Some(
                    Money::from_canonical_str(
                        "56000000000",
                        Currency::Iso(borsa_core::IsoCurrency::USD),
                    )
                    .unwrap(),
                ),
                net_income: Some(
                    Money::from_canonical_str(
                        "51000000000",
                        Currency::Iso(borsa_core::IsoCurrency::USD),
                    )
                    .unwrap(),
                ),
            },
        ])
    }
    async fn calendar(&self, _symbol: &str) -> Result<yf::fundamentals::Calendar, BorsaError> {
        Err(BorsaError::unsupported("calendar"))
    }
}

struct Combo {
    f: Arc<dyn adapter::YfFundamentals>,
}
impl adapter::CloneArcAdapters for Combo {
    fn clone_arc_fundamentals(&self) -> Arc<dyn adapter::YfFundamentals> {
        self.f.clone()
    }
    // others use default unsupported stubs
}

#[tokio::test]
async fn cashflow_uses_injected_adapter_and_maps() {
    let yf = YfConnector::from_adapter(&Combo {
        f: Arc::new(StubFundamentals),
    });
    let inst = Instrument::from_symbol("GOOG", AssetKind::Equity).expect("valid test instrument");

    let rows = yf.cashflow(&inst, true).await.unwrap();

    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[0].period,
        Period::Date(
            chrono::Utc
                .timestamp_opt(1_700_000_000, 0)
                .unwrap()
                .date_naive()
        )
    );
    assert_eq!(
        rows[0]
            .free_cash_flow
            .as_ref()
            .map(|m| m.amount().to_string())
            .as_deref(),
        Some("68000000000")
    );
}
