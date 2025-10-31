use borsa::Borsa;
use borsa_core::{AssetKind, BorsaConnector, ConnectorKey, HistoryRequest, RoutingPolicyBuilder};

use crate::helpers::{BTC_USD, m_hist};
use crate::helpers::X;

#[tokio::test]
async fn per_kind_priority_is_applied() {
    let low = m_hist("low", &[3, 4]);
    let high = m_hist("high", &[1, 2, 3, 4]);

    let policy = RoutingPolicyBuilder::new()
        .providers_for_kind(AssetKind::Crypto, &[high.key(), low.key()])
        .build();
    let borsa = Borsa::builder()
        .with_connector(low.clone())
        .with_connector(high.clone())
        .routing_policy(policy)
        .build()
        .unwrap();

    let inst = crate::helpers::instrument(&BTC_USD, AssetKind::Crypto);
    let req =
        HistoryRequest::try_from_range(borsa_core::Range::D1, borsa_core::Interval::D1).unwrap();
    let merged = borsa.history(&inst, req).await.unwrap();

    assert_eq!(merged.candles.len(), 4);
    assert_eq!(merged.candles[0].ts.timestamp(), 1);
    assert_eq!(merged.candles[3].ts.timestamp(), 4);
}

#[tokio::test]
async fn routing_policy_with_unknown_connector_fails() {
    let known = m_hist("known", &[1]);

    let policy = RoutingPolicyBuilder::new()
        .providers_global(&[ConnectorKey::new("missing-global")])
        .build();

    let Err(err) = Borsa::builder()
        .with_connector(known)
        .routing_policy(policy)
        .build()
    else {
        panic!("builder should reject unknown connector references")
    };

    let msg = err.to_string();
    assert!(
        msg.contains("unknown connectors") && msg.contains("missing-global"),
        "error should mention unknown connectors and the missing name, got: {msg}"
    );
}

#[tokio::test]
async fn routing_policy_global_strict_excludes_unlisted() {
    let known = m_hist("known", &[1, 2]);
    let other = m_hist("other", &[3]);

    let policy = RoutingPolicyBuilder::new()
        .providers_global_strict(&[known.key()])
        .build();

    let borsa = Borsa::builder()
        .with_connector(known.clone())
        .with_connector(other.clone())
        .routing_policy(policy)
        .build()
        .unwrap();

    let inst = crate::helpers::instrument(&X, AssetKind::Equity);
    let req =
        HistoryRequest::try_from_range(borsa_core::Range::D1, borsa_core::Interval::D1).unwrap();
    let merged = borsa.history(&inst, req).await.unwrap();
    // Should come only from 'known' given global strict.
    assert_eq!(merged.candles.first().unwrap().ts.timestamp(), 1);
}
