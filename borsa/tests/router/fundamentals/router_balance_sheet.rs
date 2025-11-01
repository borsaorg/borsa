use crate::helpers::{MSFT, MockConnector};
use crate::helpers::{dt, usd};
use borsa::Borsa;
use borsa_core::{AssetKind, BalanceSheetRow, BorsaError, Period};
use rust_decimal::Decimal;

#[tokio::test]
async fn balance_sheet_falls_back_and_succeeds() {
    let err = MockConnector::builder()
        .name("err_bs")
        .with_balance_sheet_fn(|_, _| Err(BorsaError::unsupported("balance_sheet")))
        .build();

    let row = BalanceSheetRow {
        period: Period::Date(dt(2023, 11, 14, 0, 0, 0).date_naive()),
        total_assets: Some(usd("412000000000")),
        total_liabilities: Some(usd("210000000000")),
        total_equity: Some(usd("202000000000")),
        cash: Some(usd("81000000000")),
        long_term_debt: Some(usd("45000000000")),
        shares_outstanding: None,
    };
    let ok_row = row.clone();
    let ok = MockConnector::builder()
        .name("ok_bs")
        .with_balance_sheet_fn(move |_, quarterly| {
            assert!(quarterly, "this test expects quarterly=true");
            Ok(vec![ok_row.clone()])
        })
        .build();

    let borsa = Borsa::builder()
        .with_connector(err)
        .with_connector(ok)
        .build()
        .unwrap();

    let inst = crate::helpers::instrument(&MSFT, AssetKind::Equity);
    let rows = borsa.balance_sheet(&inst, true).await.unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].cash.as_ref().map(borsa_core::Money::amount),
        Some(Decimal::from(81_000_000_000_u64))
    );
    assert_eq!(
        rows[0]
            .long_term_debt
            .as_ref()
            .map(borsa_core::Money::amount),
        Some(Decimal::from(45_000_000_000_u64))
    );
}
