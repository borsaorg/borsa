use borsa::Borsa;
use borsa_core::{AssetKind, HistoryRequest, Interval, Range};

use crate::helpers::m_hist;

#[tokio::test]
async fn history_with_attribution_marks_provider_spans() {
    // A covers [1,2,3], B covers [3,4,5]; first-wins on overlap (ts=3).
    let a = m_hist("A", &[1, 2, 3]);
    let b = m_hist("B", &[3, 4, 5]);

    let borsa = Borsa::builder().with_connector(a).with_connector(b).build();

    let inst = crate::helpers::instrument("BTC-USD", AssetKind::Crypto);
    let req = HistoryRequest::try_from_range(Range::D5, Interval::D1).unwrap();

    let (merged, attr) = borsa.history_with_attribution(&inst, req).await.unwrap();

    // Data merge sanity
    let ts: Vec<i64> = merged.candles.iter().map(|c| c.ts.timestamp()).collect();
    assert_eq!(ts, vec![1, 2, 3, 4, 5]);

    // Attribution sanity:
    // - A should be credited for [1..=3]
    // - B should be credited for [4..=5]
    // Note: spans are first-wins overlaid; not necessarily gap-free “continuous” checks.
    assert_eq!(attr.symbol, "BTC-USD");
    assert_eq!(attr.spans.len(), 2);

    let (n0, s0) = attr.spans[0];
    assert_eq!(n0, "A");
    assert_eq!((s0.start, s0.end), (1, 3));

    let (n1, s1) = attr.spans[1];
    assert_eq!(n1, "B");
    assert_eq!((s1.start, s1.end), (4, 5));
}
