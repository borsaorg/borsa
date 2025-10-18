#![cfg(feature = "dataframe")]

use borsa::Borsa;
use borsa_core::{AssetKind, Instrument};
use std::sync::Arc;

// Bring the extension trait into scope to enable the `.to_dataframe()` method.
// Trait is re-exported from borsa-core when the `dataframe` feature is enabled
use borsa_core::ToDataFrame;

#[tokio::test]
async fn quote_to_dataframe_smoke() {
    let connector = Arc::new(borsa_mock::MockConnector::new());
    let borsa = Borsa::builder().with_connector(connector).build().unwrap();

    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).unwrap();
    let quote = borsa.quote(&inst).await.unwrap();

    let df = quote.to_dataframe().unwrap();
    assert!(df.height() >= 1);
}
