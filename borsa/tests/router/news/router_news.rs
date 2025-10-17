use crate::helpers::{MockConnector, TSLA, dt};
use borsa::Borsa;
use borsa_core::{AssetKind, NewsArticle, NewsRequest};

#[tokio::test]
async fn news_succeeds_and_passes_request() {
    let ok = MockConnector::builder()
        .name("ok_news")
        .with_news_fn(|_i, req| {
            Ok(vec![NewsArticle {
                uuid: "1".into(),
                title: format!("Test news item {}", req.count),
                publisher: None,
                link: None,
                published_at: dt(1970, 1, 1, 0, 0, 0),
            }])
        })
        .build();

    let borsa = Borsa::builder().with_connector(ok).build().unwrap();

    let inst = crate::helpers::instrument(TSLA, AssetKind::Equity);
    let req = NewsRequest {
        count: 5,
        ..Default::default()
    };
    let articles = borsa.news(&inst, req).await.unwrap();
    assert_eq!(articles.len(), 1);
    assert_eq!(articles[0].title, "Test news item 5");
}
