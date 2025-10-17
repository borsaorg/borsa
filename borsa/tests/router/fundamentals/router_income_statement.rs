use crate::helpers::MockConnector;
use crate::helpers::{dt, usd};
use borsa::Borsa;
use borsa_core::{AssetKind, IncomeStatementRow, Period};

#[tokio::test]
async fn income_statement_falls_back_and_succeeds() {
    let err = MockConnector::builder()
        .name("err_is")
        .with_income_statement_fn(|_i, _q| {
            Err(borsa_core::BorsaError::unsupported("income_statement"))
        })
        .build();
    let ok = MockConnector::builder()
        .name("ok_is")
        .with_income_statement_fn(|_i, q| {
            assert!(!q, "this test expects annual (quarterly=false)");
            Ok(vec![IncomeStatementRow {
                period: Period::Date(dt(2023, 11, 14, 0, 0, 0).date_naive()),
                total_revenue: Some(usd("100000000000")),
                gross_profit: Some(usd("50000000000")),
                operating_income: Some(usd("20000000000")),
                net_income: Some(usd("15000000000")),
            }])
        })
        .build();

    let borsa = Borsa::builder()
        .with_connector(err)
        .with_connector(ok)
        .build();

    let inst = crate::helpers::instrument("AAPL", AssetKind::Equity);
    let rows = borsa.income_statement(&inst, false).await.unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].period,
        Period::Date(dt(2023, 11, 14, 0, 0, 0).date_naive())
    );
    assert_eq!(
        rows[0].net_income.as_ref().map(borsa_core::Money::amount),
        Some(rust_decimal::Decimal::from(15_000_000_000_u64))
    );
}
