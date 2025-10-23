use crate::Borsa;
use crate::borsa_router_method;

impl Borsa {
    borsa_router_method! {
        /// Fetch ESG sustainability scores and involvement flags for an instrument.
        ///
        /// Notes: scoring methodologies vary by provider; values are surfaced as-is
        /// without cross-provider normalization.
        method: sustainability(inst: &borsa_core::Instrument) -> borsa_core::EsgScores,
        provider: EsgProvider,
        accessor: as_esg_provider,
        capability: borsa_core::Capability::Esg,
        not_found: "esg",
        call: sustainability(inst)
    }
}
