use crate::helpers::{AAPL, MSFT, instrument, usd};
use borsa::Borsa;
use borsa_core::{AssetKind, BorsaConnector, QuoteUpdate, RoutingPolicyBuilder, Symbol};
use chrono::TimeZone;

use crate::helpers::MockConnector;

#[tokio::test]
async fn provider_with_no_assigned_symbols_not_started() {
    // Provider that supports equity but we request a crypto symbol
    let provider = MockConnector::builder()
        .name("EquityOnly")
        .supports_kind(AssetKind::Equity) // Only supports equity
        .with_stream_updates(vec![QuoteUpdate {
            symbol: AAPL.clone(),
            price: Some(usd("100.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(1, 0).unwrap(),
            volume: None,
        }])
        .build();

    let borsa = Borsa::builder()
        .with_connector(provider.clone())
        .build()
        .unwrap();

    // Request a crypto symbol - provider won't be able to provide it
    let btc = Symbol::new("BTC-USD").unwrap();
    let result = borsa
        .stream_quotes(&[instrument(&btc, AssetKind::Crypto)])
        .await;

    // Should fail to start since no provider can handle crypto
    assert!(result.is_err());
}

#[tokio::test]
async fn routing_policy_filters_symbols_per_provider() {
    // Two providers with different symbol assignments
    let p1_updates = vec![QuoteUpdate {
        symbol: AAPL.clone(),
        price: Some(usd("100.0")),
        previous_close: None,
        ts: chrono::Utc.timestamp_opt(1, 0).unwrap(),
        volume: None,
    }];

    let p2_updates = vec![QuoteUpdate {
        symbol: MSFT.clone(),
        price: Some(usd("200.0")),
        previous_close: None,
        ts: chrono::Utc.timestamp_opt(2, 0).unwrap(),
        volume: None,
    }];

    let p1 = MockConnector::builder()
        .name("P1")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(p1_updates)
        .build();

    let p2 = MockConnector::builder()
        .name("P2")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(p2_updates)
        .build();

    // Policy that assigns AAPL to P1 and MSFT to P2
    let policy = RoutingPolicyBuilder::new()
        .providers_for_symbol(&AAPL, &[p1.key()])
        .providers_for_symbol(&MSFT, &[p2.key()])
        .build();

    let borsa = Borsa::builder()
        .with_connector(p1.clone())
        .with_connector(p2.clone())
        .routing_policy(policy)
        .build()
        .unwrap();

    // Request both symbols
    let (_handle, mut rx) = borsa
        .stream_quotes(&[
            instrument(&AAPL, AssetKind::Equity),
            instrument(&MSFT, AssetKind::Equity),
        ])
        .await
        .expect("stream started");

    // Should receive both updates, one from each provider
    let mut received_symbols = vec![];
    for _ in 0..2 {
        if let Some(update) = rx.recv().await {
            received_symbols.push(update.symbol.to_string());
        }
    }

    received_symbols.sort();
    assert_eq!(received_symbols, vec!["AAPL", "MSFT"]);
}

#[tokio::test]
async fn one_provider_empty_assignment_other_succeeds() {
    // Two providers, but only one can handle the requested symbol
    let p1_updates = vec![QuoteUpdate {
        symbol: AAPL.clone(),
        price: Some(usd("100.0")),
        previous_close: None,
        ts: chrono::Utc.timestamp_opt(1, 0).unwrap(),
        volume: None,
    }];

    let p1 = MockConnector::builder()
        .name("P1")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(p1_updates)
        .build();

    // P2 only supports crypto, won't be used for equity symbols
    let p2 = MockConnector::builder()
        .name("P2")
        .supports_kind(AssetKind::Crypto)
        .with_stream_updates(vec![])
        .build();

    let policy = RoutingPolicyBuilder::new()
        .providers_for_kind(AssetKind::Equity, &[p1.key()])
        .providers_for_kind(AssetKind::Crypto, &[p2.key()])
        .build();

    let borsa = Borsa::builder()
        .with_connector(p1.clone())
        .with_connector(p2.clone())
        .routing_policy(policy)
        .build()
        .unwrap();

    // Request equity symbol - only P1 should be used
    let (handle, mut rx) = borsa
        .stream_quotes(&[instrument(&AAPL, AssetKind::Equity)])
        .await
        .expect("stream started");

    let update = rx.recv().await.expect("should receive update from P1");
    assert_eq!(update.symbol, *AAPL);
    assert_eq!(update.ts.timestamp(), 1);

    handle.stop().await;
}

#[tokio::test]
async fn symbol_filtering_results_in_empty_assignment() {
    // Provider sends updates for multiple symbols, but we only request one
    let updates = vec![
        QuoteUpdate {
            symbol: AAPL.clone(),
            price: Some(usd("100.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(1, 0).unwrap(),
            volume: None,
        },
        QuoteUpdate {
            symbol: MSFT.clone(), // We won't request this
            price: Some(usd("200.0")),
            previous_close: None,
            ts: chrono::Utc.timestamp_opt(2, 0).unwrap(),
            volume: None,
        },
    ];

    let provider = MockConnector::builder()
        .name("Multi")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(updates)
        .build();

    let borsa = Borsa::builder()
        .with_connector(provider.clone())
        .build()
        .unwrap();

    // Only request AAPL
    let (_handle, mut rx) = borsa
        .stream_quotes(&[instrument(&AAPL, AssetKind::Equity)])
        .await
        .expect("stream started");

    // Should receive AAPL
    let first = rx.recv().await.expect("should receive AAPL");
    assert_eq!(first.symbol, *AAPL);

    // MSFT should be filtered out
    let timeout_result =
        tokio::time::timeout(std::time::Duration::from_millis(50), rx.recv()).await;
    assert!(
        timeout_result.is_err(),
        "MSFT should be filtered out, no more updates expected"
    );
}

#[tokio::test]
async fn provider_with_empty_assignment_after_filtering_not_started() {
    // Provider supports multiple symbols but routing restricts it
    let aapl_update = QuoteUpdate {
        symbol: AAPL.clone(),
        price: Some(usd("100.0")),
        previous_close: None,
        ts: chrono::Utc.timestamp_opt(1, 0).unwrap(),
        volume: None,
    };

    let msft_update = QuoteUpdate {
        symbol: MSFT.clone(),
        price: Some(usd("200.0")),
        previous_close: None,
        ts: chrono::Utc.timestamp_opt(2, 0).unwrap(),
        volume: None,
    };

    // P1 can provide both AAPL and MSFT
    let p1 = MockConnector::builder()
        .name("P1")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(vec![aapl_update.clone(), msft_update.clone()])
        .build();

    // P2 can also provide both, but has higher priority for MSFT only
    let p2 = MockConnector::builder()
        .name("P2")
        .supports_kind(AssetKind::Equity)
        .with_stream_updates(vec![msft_update])
        .build();

    // Policy: AAPL -> P1, MSFT -> P2 > P1
    let policy = RoutingPolicyBuilder::new()
        .providers_for_symbol(&AAPL, &[p1.key()])
        .providers_for_symbol(&MSFT, &[p2.key(), p1.key()])
        .build();

    let borsa = Borsa::builder()
        .with_connector(p1.clone())
        .with_connector(p2.clone())
        .routing_policy(policy)
        .build()
        .unwrap();

    // Request only AAPL - P2 should not be started (has no assignment)
    let (_handle, mut rx) = borsa
        .stream_quotes(&[instrument(&AAPL, AssetKind::Equity)])
        .await
        .expect("stream started");

    // Should only receive AAPL from P1
    let update = rx.recv().await.expect("should receive AAPL from P1");
    assert_eq!(update.symbol, *AAPL);

    // No more updates since we only requested AAPL
    let timeout_result =
        tokio::time::timeout(std::time::Duration::from_millis(50), rx.recv()).await;
    assert!(timeout_result.is_err(), "no more updates expected");
}
