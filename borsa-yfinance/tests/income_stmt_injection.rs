#![cfg(feature = "test-adapters")]

use std::sync::Arc;

use async_trait::async_trait;
use borsa_core::{AssetKind, Instrument, connector::IncomeStatementProvider};
use borsa_yfinance::{YfConnector, adapter};
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
async fn income_stmt_injection_maps_correctly() {
    let yf = YfConnector::from_adapter(&Combo {
        f: Arc::new(StubFundamentals),
    });
    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid test instrument");

    // Default impl is unsupported, verify error bubbles
    let err = yf.income_statement(&inst, true).await.unwrap_err();
    assert!(format!("{err}").contains("unsupported"));
}
