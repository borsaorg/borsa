#![cfg(feature = "dataframe")]

use borsa::Borsa;
use borsa_core::{HistoryRequest, Instrument, Interval, Range};
use std::sync::Arc;

use borsa_core::ToDataFrameVec;

#[tokio::test]
async fn history_to_dataframe_smoke() {
    let connector = Arc::new(borsa_mock::MockConnector::new());
    let borsa = Borsa::builder().with_connector(connector).build().unwrap();

    let inst = Instrument::from_symbol("AAPL", borsa_core::AssetKind::Equity).unwrap();
    let req = HistoryRequest::try_from_range(Range::M1, Interval::D1).unwrap();
    let history = borsa.history(&inst, req).await.unwrap();

    let df = history.candles.to_dataframe().unwrap();
    assert!(df.height() >= 1);
}
