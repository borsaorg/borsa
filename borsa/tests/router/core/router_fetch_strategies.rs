use borsa::{Borsa, FetchStrategy};

use crate::helpers::usd;
use borsa_core::{AssetKind, Quote};
use rust_decimal::Decimal;

use crate::helpers::{MockConnector, X};
use tokio::time::{Duration, advance};

#[tokio::test]
async fn strategy_latency_returns_fastest_success() {
    let fast_ok = MockConnector::builder()
        .name("fast")
        .delay(std::time::Duration::from_millis(10))
        .returns_quote_ok(Quote {
            symbol: X.clone(),
            shortname: None,
            price: Some(usd("11.0")),
            previous_close: None,
            exchange: None,
            market_state: None,
            day_volume: None,
        })
        .build();
    let slow_ok = MockConnector::builder()
        .name("slow")
        .delay(std::time::Duration::from_millis(100))
        .returns_quote_ok(Quote {
            symbol: X.clone(),
            shortname: None,
            price: Some(usd("99.0")),
            previous_close: None,
            exchange: None,
            market_state: None,
            day_volume: None,
        })
        .build();

    let borsa = Borsa::builder()
        .with_connector(slow_ok)
        .with_connector(fast_ok)
        .fetch_strategy(FetchStrategy::Latency)
        .build()
        .unwrap();
    
    let inst = crate::helpers::instrument(&X, AssetKind::Equity);
    let q = borsa.quote(&inst).await.unwrap();
    assert_eq!(
        q.price.as_ref().map(borsa_core::Money::amount),
        Some(Decimal::from(11u8))
    );
}

#[tokio::test]
async fn strategy_latency_ignores_faster_failure_and_returns_first_success() {
    // Fail immediately faster than the successful provider
    let fast_fail = MockConnector::builder()
        .name("fast_fail")
        .delay(std::time::Duration::from_millis(5))
        .with_quote_fn(|_i| Err(borsa_core::BorsaError::Other("boom".into())))
        .build();
    let slow_ok = MockConnector::builder()
        .name("slow_ok")
        .delay(std::time::Duration::from_millis(20))
        .returns_quote_ok(Quote {
            symbol: X.clone(),
            shortname: None,
            price: Some(usd("77.0")),
            previous_close: None,
            exchange: None,
            market_state: None,
            day_volume: None,
        })
        .build();

    let borsa = Borsa::builder()
        .with_connector(fast_fail)
        .with_connector(slow_ok)
        .fetch_strategy(FetchStrategy::Latency)
        .build()
        .unwrap();

    let inst = crate::helpers::instrument(&X, AssetKind::Equity);
    let q = borsa.quote(&inst).await.unwrap();
    assert_eq!(
        q.price.as_ref().map(borsa_core::Money::amount),
        Some(Decimal::from(77u8))
    );
}

#[tokio::test(start_paused = true)]
async fn strategy_priority_with_fallback_obeys_order_and_timeout() {
    // First connector times out beyond configured threshold; second succeeds
    let very_slow = MockConnector::builder()
        .name("first")
        .delay(std::time::Duration::from_millis(200))
        .returns_quote_ok(Quote {
            symbol: X.clone(),
            shortname: None,
            price: Some(usd("1000.0")),
            previous_close: None,
            exchange: None,
            market_state: None,
            day_volume: None,
        })
        .build();
    let ok = MockConnector::builder()
        .name("second")
        .delay(std::time::Duration::from_millis(10))
        .returns_quote_ok(Quote {
            symbol: X.clone(),
            shortname: None,
            price: Some(usd("42.0")),
            previous_close: None,
            exchange: None,
            market_state: None,
            day_volume: None,
        })
        .build();

    let borsa = Borsa::builder()
        .with_connector(very_slow)
        .with_connector(ok)
        .fetch_strategy(FetchStrategy::PriorityWithFallback)
        .provider_timeout(std::time::Duration::from_millis(50))
        .build()
        .unwrap();

    let inst = crate::helpers::instrument(&X, AssetKind::Equity);
    let handle = tokio::spawn({
        let inst = inst.clone();
        async move { borsa.quote(&inst).await }
    });

    tokio::task::yield_now().await;
    advance(Duration::from_millis(50)).await; // first provider times out
    tokio::task::yield_now().await;
    advance(Duration::from_millis(10)).await; // second provider completes

    let q = handle.await.unwrap().unwrap();
    assert_eq!(
        q.price.as_ref().map(borsa_core::Money::amount),
        Some(Decimal::from(42u8))
    );
}
