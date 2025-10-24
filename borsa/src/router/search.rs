use crate::Borsa;
use crate::borsa_router_search;
use borsa_core::{Capability, SearchRequest};

impl Borsa {
    borsa_router_search! {
        /// Search for instruments using a free-text query, with optional kind filter and result limit.
        ///
        /// Behavior and trade-offs:
        /// - Executes search across eligible providers concurrently, merges results,
        ///   and de-duplicates by symbol.
        /// - If `limit` is set, truncates after merge to enforce the cap.
        method: search(req: SearchRequest) -> SearchReport,
        accessor: as_search_provider,
        capability: Capability::Search,
        call: search(req)
    }
}
