use borsa::Borsa;
use borsa_core::{AssetKind, HistoryRequest};

use crate::helpers::m_hist;

#[tokio::test]
async fn per_kind_priority_is_applied() {
    let low = m_hist("low", &[3, 4]);
    let high = m_hist("high", &[1, 2, 3, 4]);

    let borsa = Borsa::builder()
        .with_connector(low.clone())
        .with_connector(high.clone())
        .prefer_for_kind(AssetKind::Crypto, &[high, low])
        .build()
        .unwrap();

    let inst = crate::helpers::instrument("BTC-USD", AssetKind::Crypto);
    let req =
        HistoryRequest::try_from_range(borsa_core::Range::D1, borsa_core::Interval::D1).unwrap();
    let merged = borsa.history(&inst, req).await.unwrap();

    assert_eq!(merged.candles.len(), 4);
    assert_eq!(merged.candles[0].ts.timestamp(), 1);
    assert_eq!(merged.candles[3].ts.timestamp(), 4);
}
