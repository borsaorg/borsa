use crate::Borsa;
use crate::borsa_router_method;

impl Borsa {
    borsa_router_method! {
        /// List available options expiration dates for an instrument (UTC epoch seconds).
        ///
        /// Notes: expirations reflect the source's calendar; far-dated LEAPS support
        /// depends on provider coverage.
        method: options_expirations(inst: &borsa_core::Instrument) -> Vec<i64>,
        provider: OptionsExpirationsProvider,
        accessor: as_options_expirations_provider,
        capability: borsa_core::Capability::OptionsExpirations,
        not_found: "options",
        call: options_expirations(inst)
    }

    borsa_router_method! {
        /// Fetch the option chain for an instrument at an optional expiration date.
        ///
        /// Trade-offs: chains can be large; consider filtering or paging at the
        /// consumer level if your provider does not support server-side limits.
        method: option_chain(inst: &borsa_core::Instrument, date: Option<i64>) -> borsa_core::OptionChain,
        provider: OptionChainProvider,
        accessor: as_option_chain_provider,
        capability: borsa_core::Capability::OptionChain,
        not_found: "options",
        call: option_chain(inst, date)
    }
}
