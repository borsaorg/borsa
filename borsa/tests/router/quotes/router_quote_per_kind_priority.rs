use borsa::Borsa;
use borsa_core::{AssetKind, BorsaConnector, RoutingPolicyBuilder};

use crate::helpers::{X, m_quote};

#[tokio::test]
async fn per_kind_priority_applies_to_quotes() {
    let low = m_quote("low", 10.0);
    let high = m_quote("high", 99.0);

    let policy = RoutingPolicyBuilder::new()
        .providers_for_kind(AssetKind::Equity, &[high.key(), low.key()])
        .build();

    let borsa = Borsa::builder()
        .with_connector(low.clone())
        .with_connector(high.clone())
        .routing_policy(policy)
        .build()
        .unwrap();

    let inst = crate::helpers::instrument(&X, AssetKind::Equity);
    let q = borsa.quote(&inst).await.unwrap();
    assert_eq!(
        q.price.unwrap().amount(),
        rust_decimal::Decimal::from_f64_retain(99.0).unwrap()
    );
}
