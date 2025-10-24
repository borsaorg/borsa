use crate::Borsa;
use crate::borsa_router_method;
use borsa_core::{Capability, Instrument, NewsArticle, NewsRequest};

impl Borsa {
    borsa_router_method! {
        /// Fetch recent news articles for an instrument.
        ///
        /// Behavior: providers may include duplicates or syndicated content; no
        /// de-duplication beyond provider response is applied here.
        method: news(inst: &Instrument, req: NewsRequest) -> Vec<NewsArticle>,
        provider: NewsProvider,
        accessor: as_news_provider,
        capability: Capability::News,
        not_found: "news",
        call: news(inst, req)
    }
}
