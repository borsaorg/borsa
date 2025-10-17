use borsa::Borsa;
use borsa_core::{AssetKind, HistoryRequest, Interval, Range};
// use chrono::TimeZone; // no longer needed
use crate::helpers::dt;

use crate::helpers::m_hist;

#[tokio::test]
async fn history_with_valid_range_succeeds() {
    let a = m_hist("A", &[1, 2, 3]);
    let borsa = Borsa::builder().with_connector(a).build().unwrap();

    let inst = crate::helpers::instrument("BTC-USD", AssetKind::Crypto);

    let req = HistoryRequest::try_from_range(Range::M6, Interval::D1).unwrap();
    // With paft types, invalid (both range and period) cannot be constructed.
    // Router should accept a valid range request and return a response (non-empty in our mock).
    let resp = borsa
        .history(&inst, req)
        .await
        .expect("history should succeed");
    assert!(!resp.candles.is_empty());
}

#[tokio::test]
async fn history_bad_period_order_is_rejected_by_constructor() {
    // With paft types, invalid period order is rejected at construction time.
    let req = HistoryRequest::try_from_period(
        dt(1970, 1, 1, 0, 3, 20),
        dt(1970, 1, 1, 0, 1, 40),
        Interval::D1,
    );
    assert!(req.is_err());
}
