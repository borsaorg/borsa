#![cfg(feature = "test-adapters")]

use borsa_core::{AssetKind, Instrument, NewsRequest, NewsTab, connector::NewsProvider};
use borsa_yfinance::{YfConnector, adapter};
use chrono::TimeZone;

use std::sync::Arc;
use yfinance_rs as yf;

struct Combo {
    n: Arc<dyn adapter::YfNews>,
}
impl adapter::CloneArcAdapters for Combo {
    fn clone_arc_news(&self) -> Arc<dyn adapter::YfNews> {
        self.n.clone()
    }
}

#[tokio::test]
async fn news_uses_injected_adapter_and_maps() {
    let news_adapter = <dyn adapter::YfNews>::from_fn(|sym, req| {
        assert_eq!(sym, "TSLA");
        assert_eq!(req.count, 5);
        assert_eq!(req.tab, NewsTab::PressReleases);
        Ok(vec![yf::news::NewsArticle {
            uuid: "123".into(),
            title: "Cybertruck recall".into(),
            publisher: Some("Reuters".into()),
            link: None,
            published_at: chrono::Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        }])
    });

    let yf = YfConnector::from_adapter(&Combo { n: news_adapter });
    let inst = Instrument::from_symbol("TSLA", AssetKind::Equity).expect("valid test instrument");
    let req = NewsRequest {
        count: 5,
        tab: NewsTab::PressReleases,
    };

    let articles = yf.news(&inst, req).await.unwrap();
    assert_eq!(articles.len(), 1);
    assert_eq!(articles[0].title, "Cybertruck recall");
}
