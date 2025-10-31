use borsa::Borsa;
use borsa_core::{AssetKind, BorsaError, QuoteUpdate};
use chrono::TimeZone;

use crate::helpers::{AAPL, BTC_USD, MockConnector, usd};

#[tokio::test]
async fn stream_quotes_errors_when_all_providers_fail_to_start() {
    let first = MockConnector::builder()
        .name("fail_primary")
        .will_fail_stream_start("primary failed")
        .build();
    let second = MockConnector::builder()
        .name("fail_secondary")
        .will_fail_stream_start("secondary failed")
        .build();

    let borsa = Borsa::builder()
        .with_connector(first)
        .with_connector(second)
        .build()
        .unwrap();

    let inst = crate::helpers::instrument(&AAPL, AssetKind::Equity);

    let err = borsa.stream_quotes(&[inst]).await.unwrap_err();

    match err {
        BorsaError::AllProvidersFailed(errors) => {
            assert_eq!(errors.len(), 2);
            for e in errors {
                match e {
                    BorsaError::Connector { connector, .. } => {
                        assert!(connector == "fail_primary" || connector == "fail_secondary");
                    }
                    other => panic!("expected connector error, got {other:?}"),
                }
            }
        }
        other => panic!("expected AllProvidersFailed, got {other:?}"),
    }
}

#[tokio::test]
async fn stream_quotes_errors_when_one_kind_fails_to_start() {
    let equity_updates = vec![QuoteUpdate {
        symbol: AAPL.clone(),
        price: Some(usd("120.0")),
        previous_close: None,
        ts: chrono::Utc.timestamp_opt(10, 0).unwrap(),
        volume: None,
    }];
    let equities = MockConnector::builder()
        .name("equity_ok")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(equity_updates)
        .build();
    let crypto = MockConnector::builder()
        .name("crypto_fail")
        .supports_kind(AssetKind::Crypto)
        .will_fail_stream_start("crypto stream failed")
        .build();

    let borsa = Borsa::builder()
        .with_connector(equities)
        .with_connector(crypto)
        .build()
        .unwrap();

    let equity = crate::helpers::instrument(&AAPL, AssetKind::Equity);
    let crypto = crate::helpers::instrument(&BTC_USD, AssetKind::Crypto);

    let err = borsa.stream_quotes(&[equity, crypto]).await.unwrap_err();

    match err {
        BorsaError::Connector { connector, msg } => {
            assert_eq!(connector, "crypto_fail");
            assert!(
                msg.contains("crypto stream failed"),
                "unexpected connector message: {msg}"
            );
        }
        other => panic!("expected connector error, got {other:?}"),
    }
}
