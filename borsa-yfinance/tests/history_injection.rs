#![cfg(feature = "test-adapters")]

use std::sync::Arc;

use borsa_core::{AssetKind, HistoryRequest, Instrument, connector::HistoryProvider};
use borsa_core::{Currency, Money};
use borsa_yfinance::{YfConnector, adapter};
use chrono::{TimeZone, Utc};
use yfinance_rs as yf;

// Bundle both trait objects into something that satisfies CloneArcAdapters.
struct Combo {
    h: Arc<dyn adapter::YfHistory>,
}
impl adapter::CloneArcAdapters for Combo {
    fn clone_arc_history(&self) -> Arc<dyn adapter::YfHistory> {
        self.h.clone()
    }
}

#[tokio::test]
async fn history_uses_injected_adapter() {
    // Build a fake history adapter (no network).
    let hist = <dyn adapter::YfHistory>::from_fn(|symbol, req| {
        assert_eq!(symbol, "BTC-USD");
        assert!(req.auto_adjust);

        let candles = vec![
            yf::Candle {
                ts: Utc.timestamp_opt(1, 0).unwrap(),
                open: Money::from_canonical_str("1.0", Currency::Iso(borsa_core::IsoCurrency::USD))
                    .unwrap(),
                high: Money::from_canonical_str("1.0", Currency::Iso(borsa_core::IsoCurrency::USD))
                    .unwrap(),
                low: Money::from_canonical_str("1.0", Currency::Iso(borsa_core::IsoCurrency::USD))
                    .unwrap(),
                close: Money::from_canonical_str(
                    "1.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                close_unadj: None,
                volume: None,
            },
            yf::Candle {
                ts: Utc.timestamp_opt(2, 0).unwrap(),
                open: Money::from_canonical_str("2.0", Currency::Iso(borsa_core::IsoCurrency::USD))
                    .unwrap(),
                high: Money::from_canonical_str("2.0", Currency::Iso(borsa_core::IsoCurrency::USD))
                    .unwrap(),
                low: Money::from_canonical_str("2.0", Currency::Iso(borsa_core::IsoCurrency::USD))
                    .unwrap(),
                close: Money::from_canonical_str(
                    "2.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                close_unadj: None,
                volume: None,
            },
        ];
        let raw = yf::HistoryResponse {
            candles,
            actions: vec![],
            adjusted: true,
            meta: Some(yf::HistoryMeta {
                timezone: None,
                utc_offset_seconds: Some(0),
            }),
        };
        Ok(raw)
    });

    let connector = YfConnector::from_adapter(&Combo { h: hist });

    let inst =
        Instrument::from_symbol("BTC-USD", AssetKind::Crypto).expect("valid test instrument");
    let resp = connector
        .history(
            &inst,
            HistoryRequest::try_from_range(borsa_core::Range::M1, borsa_core::Interval::D1)
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.candles.len(), 2);
    assert_eq!(resp.candles[0].ts, Utc.timestamp_opt(1, 0).unwrap());
    assert!(resp.adjusted);
}
