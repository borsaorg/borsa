use borsa::Borsa;

use borsa_core::{AssetKind, BorsaError};
use rust_decimal::Decimal;

use crate::helpers::{MockConnector, m_quote, X};

#[tokio::test]
async fn all_not_found_returns_not_found() {
    let nf = MockConnector::builder()
        .name("nf")
        .with_quote_fn(|_i| Err(borsa_core::BorsaError::not_found("quote for X")))
        .build();
    let borsa = Borsa::builder().with_connector(nf).build().unwrap();

    let inst = crate::helpers::instrument(&X, AssetKind::Equity);
    let err = borsa.quote(&inst).await.expect_err("should error");
    assert!(matches!(err, BorsaError::NotFound { .. }));
}

#[tokio::test]
async fn not_found_does_not_block_success() {
    let nf = MockConnector::builder()
        .name("nf")
        .with_quote_fn(|_i| Err(borsa_core::BorsaError::not_found("quote for X")))
        .build();
    let borsa = Borsa::builder()
        .with_connector(nf) // returns NotFound
        .with_connector(m_quote("ok", 42.0)) // returns success
        .build()
        .unwrap();

    let inst = crate::helpers::instrument(&X, AssetKind::Equity);
    let q = borsa.quote(&inst).await.unwrap();
    assert_eq!(
        q.price.as_ref().map(borsa_core::Money::amount).unwrap(),
        Decimal::from(42u8)
    );
}
