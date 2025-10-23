use crate::Borsa;
use crate::borsa_router_search;

impl Borsa {
    borsa_router_search! {
        /// Search for instruments using a free-text query, with optional kind filter and result limit.
        ///
        /// Behavior and trade-offs:
        /// - Executes search across eligible providers concurrently, merges results,
        ///   and de-duplicates by symbol.
        /// - If `limit` is set, truncates after merge to enforce the cap.
        method: search(req: borsa_core::SearchRequest) -> borsa_core::SearchReport,
        accessor: as_search_provider,
        capability: borsa_core::Capability::Search,
        call: search(req)
    }
}
