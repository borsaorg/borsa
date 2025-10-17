#![cfg(feature = "test-adapters")]

use std::sync::Arc;

use borsa_core::{AssetKind, BorsaError, Instrument, connector::QuoteProvider};
use borsa_yfinance::{YfConnector, adapter};

struct Combo {
    q: Arc<dyn adapter::YfQuotes>,
}
impl adapter::CloneArcAdapters for Combo {
    fn clone_arc_quotes(&self) -> Arc<dyn adapter::YfQuotes> {
        self.q.clone()
    }
}

#[tokio::test]
async fn connector_other_error_preserves_connector_name() {
    let quotes = <dyn adapter::YfQuotes>::from_fn(|_symbols| {
        Err(BorsaError::Other("some http error".to_string()))
    });

    let yf = YfConnector::from_adapter(&Combo { q: quotes });

    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid test instrument");
    let err = yf.quote(&inst).await.unwrap_err();
    match err {
        BorsaError::Connector { connector, .. } => assert_eq!(connector, "borsa-yfinance"),
        _ => panic!("expected connector error"),
    }
}

#[tokio::test]
async fn not_found_maps_to_not_found() {
    let quotes = <dyn adapter::YfQuotes>::from_fn(|_symbols| {
        Err(BorsaError::Connector {
            connector: "borsa-yfinance".into(),
            msg: "Not Found".into(),
        })
    });

    let yf = YfConnector::from_adapter(&Combo { q: quotes });

    let inst = Instrument::from_symbol("ZZZ", AssetKind::Equity).expect("valid test instrument");
    let err = yf.quote(&inst).await.unwrap_err();
    assert!(matches!(err, BorsaError::NotFound { .. }));
}

#[tokio::test]
async fn rate_limited_preserves_connector() {
    let quotes = <dyn adapter::YfQuotes>::from_fn(|_symbols| {
        Err(BorsaError::Connector {
            connector: "borsa-yfinance".into(),
            msg: "rate limit".into(),
        })
    });

    let yf = YfConnector::from_adapter(&Combo { q: quotes });

    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid test instrument");
    let err = yf.quote(&inst).await.unwrap_err();
    match err {
        BorsaError::Connector { connector, .. } => assert_eq!(connector, "borsa-yfinance"),
        _ => panic!("expected connector error"),
    }
}

#[tokio::test]
async fn server_status_maps_to_connector() {
    let quotes = <dyn adapter::YfQuotes>::from_fn(|_symbols| {
        Err(BorsaError::Connector {
            connector: "borsa-yfinance".into(),
            msg: "server error 500".into(),
        })
    });

    let yf = YfConnector::from_adapter(&Combo { q: quotes });

    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity).expect("valid test instrument");
    let err = yf.quote(&inst).await.unwrap_err();
    match err {
        BorsaError::Connector { connector, .. } => assert_eq!(connector, "borsa-yfinance"),
        _ => panic!("expected connector error"),
    }
}
