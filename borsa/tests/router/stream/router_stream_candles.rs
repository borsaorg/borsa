use crate::helpers::{AAPL, candle, instrument};
use borsa_core::{AssetKind, BorsaConnector, CandleUpdate, Interval, RoutingPolicyBuilder};

use crate::helpers::MockConnector;

#[tokio::test]
async fn stream_candles_routes_to_native_candle_connector() {
    let a = MockConnector::builder()
        .name("A")
        .supports_kind(AssetKind::Equity)
        .build();

    let interval = Interval::I1h;
    let update1 = CandleUpdate {
        instrument: instrument(&AAPL, AssetKind::Equity),
        interval,
        candle: candle(1, 200.0),
        is_final: false,
    };
    let update2 = CandleUpdate {
        instrument: instrument(&AAPL, AssetKind::Equity),
        interval,
        candle: candle(2, 201.5),
        is_final: true,
    };
    let b = MockConnector::builder()
        .name("B")
        .supports_kind(AssetKind::Equity)
        .with_candle_stream_updates(vec![update1.clone(), update2.clone()])
        .build();

    let policy = RoutingPolicyBuilder::new()
        .providers_for_symbol(&AAPL, &[b.key(), a.key()])
        .build();
    let borsa = borsa::Borsa::builder()
        .with_connector(a.clone())
        .with_connector(b.clone())
        .routing_policy(policy)
        .build()
        .unwrap();

    let (_handle, mut rx) = borsa
        .stream_candles(&[instrument(&AAPL, AssetKind::Equity)], interval)
        .await
        .expect("stream started");

    let first = rx.recv().await.expect("first update");
    assert_eq!(first, update1);

    let second = rx.recv().await.expect("second update");
    assert_eq!(second, update2);
}
