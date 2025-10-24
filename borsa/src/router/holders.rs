use crate::Borsa;
use crate::borsa_router_method;
use borsa_core::{
    Capability, InsiderRosterHolder, InsiderTransaction, InstitutionalHolder, Instrument,
    MajorHolder, NetSharePurchaseActivity,
};

impl Borsa {
    borsa_router_method! {
        /// Fetch percentages for major holder categories.
        ///
        /// Notes: category definitions may vary across providers; values are passed
        /// through without normalization.
        method: major_holders(inst: &Instrument) -> Vec<MajorHolder>,
        provider: MajorHoldersProvider,
        accessor: as_major_holders_provider,
        capability: Capability::MajorHolders,
        not_found: "holders",
        call: major_holders(inst)
    }

    borsa_router_method! {
        /// Fetch institutional holders.
        ///
        /// Behavior: lists can be large; pagination is handled by providers when
        /// present and results are combined.
        method: institutional_holders(inst: &Instrument) -> Vec<InstitutionalHolder>,
        provider: InstitutionalHoldersProvider,
        accessor: as_institutional_holders_provider,
        capability: Capability::InstitutionalHolders,
        not_found: "holders",
        call: institutional_holders(inst)
    }

    borsa_router_method! {
        /// Fetch mutual fund holders.
        method: mutual_fund_holders(inst: &Instrument) -> Vec<InstitutionalHolder>,
        provider: MutualFundHoldersProvider,
        accessor: as_mutual_fund_holders_provider,
        capability: Capability::MutualFundHolders,
        not_found: "holders",
        call: mutual_fund_holders(inst)
    }

    borsa_router_method! {
        /// Fetch insider transactions.
        ///
        /// Notes: reported insider activity may be delayed; fields reflect provider
        /// disclosures and are not audited.
        method: insider_transactions(inst: &Instrument) -> Vec<InsiderTransaction>,
        provider: InsiderTransactionsProvider,
        accessor: as_insider_transactions_provider,
        capability: Capability::InsiderTransactions,
        not_found: "holders",
        call: insider_transactions(inst)
    }

    borsa_router_method! {
        /// Fetch the insider roster.
        method: insider_roster_holders(inst: &Instrument) -> Vec<InsiderRosterHolder>,
        provider: InsiderRosterHoldersProvider,
        accessor: as_insider_roster_holders_provider,
        capability: Capability::InsiderRoster,
        not_found: "holders",
        call: insider_roster_holders(inst)
    }

    borsa_router_method! {
        /// Fetch net share purchase activity summary for insiders.
        ///
        /// Behavior: returns `None` when a provider offers no aggregate; consumers
        /// should handle optionality.
        method: net_share_purchase_activity(inst: &Instrument) -> Option<NetSharePurchaseActivity>,
        provider: NetSharePurchaseActivityProvider,
        accessor: as_net_share_purchase_activity_provider,
        capability: Capability::NetSharePurchaseActivity,
        not_found: "holders",
        call: net_share_purchase_activity(inst)
    }
}
