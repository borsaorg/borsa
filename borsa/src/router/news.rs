use crate::Borsa;
use crate::borsa_router_method;

impl Borsa {
    borsa_router_method! {
        /// Fetch recent news articles for an instrument.
        ///
        /// Behavior: providers may include duplicates or syndicated content; no
        /// de-duplication beyond provider response is applied here.
        method: news(inst: &borsa_core::Instrument, req: borsa_core::NewsRequest) -> Vec<borsa_core::NewsArticle>,
        provider: NewsProvider,
        accessor: as_news_provider,
        capability: "news",
        not_found: "news",
        call: news(inst, req)
    }
}
