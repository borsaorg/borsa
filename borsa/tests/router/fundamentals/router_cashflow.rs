use crate::helpers::MockConnector;
use crate::helpers::{GOOG, dt, usd};
use borsa::Borsa;
use borsa_core::{AssetKind, CashflowRow, Period};
use rust_decimal::Decimal;

#[tokio::test]
async fn cashflow_falls_back_and_succeeds() {
    let err = MockConnector::builder()
        .name("err_cf")
        .with_cashflow_fn(|_i, _q| Err(borsa_core::BorsaError::unsupported("cashflow")))
        .build();
    let ok = MockConnector::builder()
        .name("ok_cf")
        .with_cashflow_fn(|_i, q| {
            assert!(q, "this test expects quarterly=true");
            Ok(vec![
                CashflowRow {
                    period: Period::Date(dt(2023, 11, 14, 0, 0, 0).date_naive()),
                    operating_cashflow: Some(usd("99000000000")),
                    capital_expenditures: Some(usd("-31000000000")),
                    free_cash_flow: Some(usd("68000000000")),
                    net_income: Some(usd("60000000000")),
                },
                CashflowRow {
                    period: Period::Date(dt(2023, 7, 15, 0, 0, 0).date_naive()),
                    operating_cashflow: Some(usd("85000000000")),
                    capital_expenditures: Some(usd("-29000000000")),
                    free_cash_flow: Some(usd("56000000000")),
                    net_income: Some(usd("51000000000")),
                },
            ])
        })
        .build();

    let borsa = Borsa::builder()
        .with_connector(err)
        .with_connector(ok)
        .build();

    let inst = crate::helpers::instrument(GOOG, AssetKind::Equity);
    let rows = borsa.cashflow(&inst, true).await.unwrap();

    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[0]
            .free_cash_flow
            .as_ref()
            .map(borsa_core::Money::amount),
        Some(Decimal::from(68_000_000_000_u64))
    );
    assert_eq!(
        rows[1]
            .operating_cashflow
            .as_ref()
            .map(borsa_core::Money::amount),
        Some(Decimal::from(85_000_000_000_u64))
    );
}
