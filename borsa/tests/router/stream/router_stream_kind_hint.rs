use crate::helpers::{AAPL, usd};
use borsa_core::{AssetKind, QuoteUpdate};
use chrono::TimeZone;

use crate::helpers::MockConnector;

#[tokio::test]
async fn stream_quotes_respects_kind_hint_support() {
    // Provider that streams but does NOT support Equity when asked via kind hint
    let wrong_kind = MockConnector::builder()
        .name("W")
        .supports_kind(AssetKind::Crypto)
        .with_stream_updates(vec![QuoteUpdate {
            symbol: borsa_core::Symbol::new(AAPL).unwrap(),
            price: Some(usd("1.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(1, 0).unwrap(),
        }])
        .build();

    // Provider that supports Equity and emits data
    let right_kind = MockConnector::builder()
        .name("R")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(vec![QuoteUpdate {
            symbol: borsa_core::Symbol::new(AAPL).unwrap(),
            price: Some(usd("2.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(2, 0).unwrap(),
        }])
        .build();

    let borsa = borsa::Borsa::builder()
        .with_connector(wrong_kind.clone())
        .with_connector(right_kind.clone())
        .prefer_for_kind(AssetKind::Equity, &[wrong_kind, right_kind]) // Even if W is first, kind hint should filter it out
        .build()
        .unwrap();

    let (_h, mut rx) = borsa
        .stream_quotes(&[crate::helpers::instrument(AAPL, AssetKind::Equity)])
        .await
        .expect("stream started");

    let u = rx.recv().await.expect("first update");
    assert_eq!(u.ts.timestamp(), 2);
    assert_eq!(u.price.unwrap().amount().to_string(), "2.0");
}
