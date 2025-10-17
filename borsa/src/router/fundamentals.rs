use crate::Borsa;
use crate::borsa_router_method;

impl Borsa {
    borsa_router_method! {
        /// Fetch earnings datasets (yearly, quarterly, EPS history).
        ///
        /// Behavior: returns combined structures if available; subfields may be empty
        /// depending on provider coverage and symbol history.
        method: earnings(inst: &borsa_core::Instrument) -> borsa_core::Earnings,
        provider: EarningsProvider,
        accessor: as_earnings_provider,
        capability: "earnings",
        not_found: "earnings",
        call: earnings(inst)
    }

    borsa_router_method! {
        /// Fetch income statement rows; set `quarterly = true` for quarterly cadence.
        ///
        /// Trade-offs: annual vs quarterly cadence alters row density; some providers
        /// report trailing values or partial periods which are passed through.
        method: income_statement(inst: &borsa_core::Instrument, quarterly: bool) -> Vec<borsa_core::IncomeStatementRow>,
        provider: IncomeStatementProvider,
        accessor: as_income_statement_provider,
        capability: "income-statement",
        not_found: "fundamentals",
        call: income_statement(inst, quarterly)
    }

    borsa_router_method! {
        /// Fetch balance sheet rows; set `quarterly = true` for quarterly cadence.
        ///
        /// Notes: units and field coverage can vary by provider; values are relayed
        /// as-is without currency normalization.
        method: balance_sheet(inst: &borsa_core::Instrument, quarterly: bool) -> Vec<borsa_core::BalanceSheetRow>,
        provider: BalanceSheetProvider,
        accessor: as_balance_sheet_provider,
        capability: "balance-sheet",
        not_found: "fundamentals",
        call: balance_sheet(inst, quarterly)
    }

    borsa_router_method! {
        /// Fetch cashflow rows; set `quarterly = true` for quarterly cadence.
        ///
        /// Behavior: direct mapping from provider; sign conventions follow the source
        /// and are not adjusted.
        method: cashflow(inst: &borsa_core::Instrument, quarterly: bool) -> Vec<borsa_core::CashflowRow>,
        provider: CashflowProvider,
        accessor: as_cashflow_provider,
        capability: "cashflow",
        not_found: "fundamentals",
        call: cashflow(inst, quarterly)
    }

    borsa_router_method! {
        /// Fetch corporate calendar entries (earnings dates and dividends).
        ///
        /// Notes: dates are UTC seconds; dividend information may be missing or
        /// delayed depending on provider refresh schedules.
        method: calendar(inst: &borsa_core::Instrument) -> borsa_core::Calendar,
        provider: CalendarProvider,
        accessor: as_calendar_provider,
        capability: "calendar",
        not_found: "calendar",
        call: calendar(inst)
    }
}
