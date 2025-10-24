use borsa_core::{Capability, Instrument, Isin, Profile};

use crate::Borsa;
use crate::borsa_router_method;

impl Borsa {
    borsa_router_method! {
        /// Fetch a company or fund profile for an instrument.
        ///
        /// Behavior: maps provider-specific entities into a normalized enum. Some
        /// fields (e.g., website, address) may be missing depending on coverage.
        method: profile(inst: &Instrument) -> Profile,
        provider: ProfileProvider,
        accessor: as_profile_provider,
        capability: Capability::Profile,
        not_found: "profile",
        call: profile(inst)
    }

    borsa_router_method! {
        /// Resolve ISIN for an instrument when available.
        ///
        /// Notes: not all providers support ISIN resolution; when unavailable, callers
        /// can derive from `profile` if the provider exposes it there.
        method: isin(inst: &Instrument) -> Option<Isin>,
        provider: IsinProvider,
        accessor: as_isin_provider,
        capability: Capability::Isin,
        not_found: "isin",
        call: isin(inst)
    }
}
