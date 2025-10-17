#![cfg(feature = "test-adapters")]

use std::sync::Arc;

use borsa_core::{
    AssetKind, HistoryRequest, Instrument, Interval, Range, connector::HistoryProvider,
};
use borsa_yfinance::{YfConnector, adapter};

struct Combo {
    h: Arc<dyn adapter::YfHistory>,
}
impl adapter::CloneArcAdapters for Combo {
    fn clone_arc_history(&self) -> Arc<dyn adapter::YfHistory> {
        self.h.clone()
    }
}

#[tokio::test]
async fn connector_accepts_valid_history_request() {
    // Build a noop adapter; request validation is handled by the type constructor.
    let hist = <dyn adapter::YfHistory>::from_fn(|_symbol, _req| {
        use chrono::TimeZone;
        Ok(borsa_core::HistoryResponse {
            candles: vec![
                borsa_core::Candle {
                    ts: chrono::Utc.timestamp_opt(1, 0).unwrap(),
                    open: borsa_core::Money::from_canonical_str(
                        "1.0",
                        borsa_core::Currency::Iso(borsa_core::IsoCurrency::USD),
                    )
                    .unwrap(),
                    high: borsa_core::Money::from_canonical_str(
                        "1.0",
                        borsa_core::Currency::Iso(borsa_core::IsoCurrency::USD),
                    )
                    .unwrap(),
                    low: borsa_core::Money::from_canonical_str(
                        "1.0",
                        borsa_core::Currency::Iso(borsa_core::IsoCurrency::USD),
                    )
                    .unwrap(),
                    close: borsa_core::Money::from_canonical_str(
                        "1.0",
                        borsa_core::Currency::Iso(borsa_core::IsoCurrency::USD),
                    )
                    .unwrap(),
                    close_unadj: None,
                    volume: None,
                },
                borsa_core::Candle {
                    ts: chrono::Utc.timestamp_opt(2, 0).unwrap(),
                    open: borsa_core::Money::from_canonical_str(
                        "2.0",
                        borsa_core::Currency::Iso(borsa_core::IsoCurrency::USD),
                    )
                    .unwrap(),
                    high: borsa_core::Money::from_canonical_str(
                        "2.0",
                        borsa_core::Currency::Iso(borsa_core::IsoCurrency::USD),
                    )
                    .unwrap(),
                    low: borsa_core::Money::from_canonical_str(
                        "2.0",
                        borsa_core::Currency::Iso(borsa_core::IsoCurrency::USD),
                    )
                    .unwrap(),
                    close: borsa_core::Money::from_canonical_str(
                        "2.0",
                        borsa_core::Currency::Iso(borsa_core::IsoCurrency::USD),
                    )
                    .unwrap(),
                    close_unadj: None,
                    volume: None,
                },
            ],
            actions: vec![],
            adjusted: false,
            meta: None,
        })
    });

    let connector = YfConnector::from_adapter(&Combo { h: hist });

    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid test instrument");
    let req = HistoryRequest::try_from_range(Range::M1, Interval::D1).unwrap();

    let ok = connector.history(&inst, req).await;
    assert!(ok.is_ok());
}
