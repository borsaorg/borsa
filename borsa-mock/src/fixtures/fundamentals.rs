use borsa_core::{BalanceSheetRow, CashflowRow, Earnings, IncomeStatementRow};

pub const fn earnings_by_symbol(_s: &str) -> Option<Earnings> {
    Some(Earnings {
        yearly: vec![],
        quarterly: vec![],
        quarterly_eps: vec![],
    })
}

pub const fn income_stmt_by_symbol(_s: &str) -> Vec<IncomeStatementRow> {
    vec![]
}
pub const fn balance_sheet_by_symbol(_s: &str) -> Vec<BalanceSheetRow> {
    vec![]
}
pub const fn cashflow_by_symbol(_s: &str) -> Vec<CashflowRow> {
    vec![]
}
