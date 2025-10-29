use crate::helpers::{AAPL, instrument};
use borsa_core::{AssetKind, RoutingPolicyBuilder};

use crate::helpers::MockConnector;

#[tokio::test]
async fn stream_quotes_strict_symbols_rejected_fails_fast() {
    // One streaming-capable connector for Equities
    let c = MockConnector::builder()
        .name("S")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(vec![])
        .build();

    // Strict rule for AAPL with an empty provider list: excludes all providers by policy.
    let policy = RoutingPolicyBuilder::new()
        .providers_rule(
            borsa_core::Selector {
                symbol: Some(AAPL.to_string()),
                kind: Some(AssetKind::Equity),
                exchange: None,
            },
            &[],
            true,
        )
        .build();

    let borsa = borsa::Borsa::builder()
        .with_connector(c.clone())
        .routing_policy(policy)
        .build()
        .unwrap();

    let err = borsa
        .stream_quotes(&[instrument(AAPL, AssetKind::Equity)])
        .await
        .expect_err("strict rejection should error");

    match err {
        borsa_core::BorsaError::StrictSymbolsRejected { rejected } => {
            assert_eq!(rejected, vec![AAPL.to_string()]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
