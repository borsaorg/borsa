use borsa::Borsa;
use borsa_core::AssetKind;

use crate::helpers::m_quote;

#[tokio::test]
async fn quote_fallback_first_success() {
    let c1 = m_quote("a", 0.0);
    let c2 = m_quote("b", 42.0);

    let borsa = Borsa::builder()
        .with_connector(c1)
        .with_connector(c2)
        .build();

    let inst = crate::helpers::instrument("X", AssetKind::Equity);
    let q = borsa.quote(&inst).await.unwrap();
    assert_eq!(
        q.price.unwrap().amount(),
        rust_decimal::Decimal::from_f64_retain(0.0).unwrap()
    );
}

#[tokio::test]
async fn quote_respects_priority_override() {
    let c1 = m_quote("low", 10.0);
    let c2 = m_quote("high", 99.0);

    let borsa = Borsa::builder()
        .with_connector(c1.clone())
        .with_connector(c2.clone())
        .prefer_symbol("X", &[c2, c1])
        .build();

    let inst = crate::helpers::instrument("X", AssetKind::Equity);
    let q = borsa.quote(&inst).await.unwrap();
    assert_eq!(
        q.price.unwrap().amount(),
        rust_decimal::Decimal::from_f64_retain(99.0).unwrap()
    );
}
