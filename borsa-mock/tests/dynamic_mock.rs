use borsa_core::{AssetKind, BorsaError, Instrument, Quote, QuoteUpdate, Symbol};
use borsa_mock::{DynamicMockConnector, MockBehavior, StreamBehavior};

fn inst(sym: &Symbol) -> Instrument {
    Instrument::from_symbol(sym, AssetKind::Equity).expect("valid symbol")
}

#[tokio::test]
async fn test_mock_quote_return() {
    let (mock, controller) = DynamicMockConnector::new_with_controller("P0");
    let sym = Symbol::new("AAPL").unwrap();
    let q = Quote {
        symbol: sym.clone(),
        shortname: None,
        price: None,
        previous_close: None,
        exchange: None,
        market_state: None,
        day_volume: None,
    };
    controller
        .set_quote_behavior(sym.clone(), MockBehavior::Return(q.clone()))
        .await;

    let qp = mock.as_quote_provider().expect("quote provider");
    let got = qp.quote(&inst(&sym)).await.expect("quote ok");
    assert_eq!(got.symbol, q.symbol);
}

#[tokio::test]
async fn test_mock_quote_fail() {
    let (mock, controller) = DynamicMockConnector::new_with_controller("P0");
    let sym = Symbol::new("MSFT").unwrap();
    let err = BorsaError::Other("boom".to_string());
    controller
        .set_quote_behavior(sym.clone(), MockBehavior::Fail(err.clone()))
        .await;

    let qp = mock.as_quote_provider().expect("quote provider");
    let got = qp.quote(&inst(&sym)).await.expect_err("err");
    assert_eq!(got, err);
}

#[tokio::test]
async fn test_mock_stream_startup_fail() {
    let (mock, controller) = DynamicMockConnector::new_with_controller("P0");
    controller
        .set_stream_behavior("P0", StreamBehavior::Fail(BorsaError::Other("nope".into())))
        .await;

    let sp = mock.as_stream_provider().expect("stream provider");
    let sym = Symbol::new("AAPL").unwrap();
    let err = sp.stream_quotes(&[inst(&sym)]).await.expect_err("err");
    assert!(matches!(err, BorsaError::Other(_)));
}

#[tokio::test]
async fn test_mock_stream_logs_requests() {
    let (mock, controller) = DynamicMockConnector::new_with_controller("P0");
    controller
        .set_stream_behavior("P0", StreamBehavior::Fail(BorsaError::Other("deny".into())))
        .await;
    let sp = mock.as_stream_provider().expect("stream provider");
    let sym = Symbol::new("AAPL").unwrap();
    let _ = sp.stream_quotes(&[inst(&sym)]).await;

    let reqs = controller.get_stream_requests("P0").await;
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].len(), 1);
    let got_sym = match reqs[0][0].id() {
        borsa_core::IdentifierScheme::Security(sec) => &sec.symbol,
        borsa_core::IdentifierScheme::Prediction(_) => {
            panic!("unexpected non-security instrument in mock test")
        }
    };
    assert_eq!(got_sym, &sym);
}

#[tokio::test]
async fn test_mock_stream_remote_kill() {
    let (mock, controller) = DynamicMockConnector::new_with_controller("P0");
    let sym = Symbol::new("AAPL").unwrap();
    let updates = vec![QuoteUpdate {
        symbol: sym.clone(),
        price: None,
        previous_close: None,
        ts: chrono::Utc::now(),
        volume: None,
    }];
    controller
        .set_stream_behavior("P0", StreamBehavior::Success(updates))
        .await;

    let sp = mock.as_stream_provider().expect("stream provider");
    let (handle, mut rx) = sp
        .stream_quotes(&[inst(&sym)])
        .await
        .expect("stream started");

    controller.fail_stream("P0").await;
    // Channel should be closed after remote kill
    let closed = rx.recv().await.is_none();
    assert!(closed);

    // Ensure handle can be stopped cleanly
    handle.stop().await;
}
