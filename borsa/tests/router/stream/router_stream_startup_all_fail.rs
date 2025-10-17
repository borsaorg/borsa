use borsa::Borsa;
use borsa_core::{AssetKind, BorsaError};

use crate::helpers::{AAPL, MockConnector};

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

    let inst = crate::helpers::instrument(AAPL, AssetKind::Equity);

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
