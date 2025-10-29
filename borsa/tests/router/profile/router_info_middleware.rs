use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use borsa::Borsa;
use borsa_core::{
    connector::BorsaConnector, AssetKind, BorsaError, CompanyProfile, Instrument, PriceTarget,
    Profile, Quote, RecommendationSummary,
};
use borsa_middleware::ConnectorBuilder;
use borsa_mock::MockConnector;
use borsa_types::{QuotaConfig, QuotaConsumptionStrategy};

use crate::helpers::usd;

fn instrument() -> Instrument {
    crate::helpers::instrument("TEST", AssetKind::Equity)
}

fn quota_config(limit: u64) -> QuotaConfig {
    QuotaConfig {
        limit,
        window: Duration::from_secs(3_600),
        strategy: QuotaConsumptionStrategy::Unit,
    }
}

fn base_quote() -> Quote {
    Quote {
        symbol: borsa_core::Symbol::new("TEST").unwrap(),
        shortname: Some("Test Corp".into()),
        price: Some(usd("123.45")),
        previous_close: None,
        exchange: None,
        market_state: None,
        day_volume: None,
    }
}

fn base_profile() -> Profile {
    Profile::Company(CompanyProfile {
        name: "Test Corporation".into(),
        summary: None,
        website: None,
        address: None,
        sector: None,
        industry: None,
        isin: None,
    })
}

fn base_recommendation_summary() -> RecommendationSummary {
    RecommendationSummary {
        latest_period: None,
        strong_buy: None,
        buy: None,
        hold: None,
        sell: None,
        strong_sell: None,
        mean: None,
        mean_rating_text: None,
    }
}

fn base_price_target() -> PriceTarget {
    PriceTarget {
        low: None,
        mean: Some(usd("150.0")),
        high: None,
        number_of_analysts: None,
    }
}

fn quota_wrapped_connector(limit: u64) -> Arc<dyn BorsaConnector> {
    let raw: Arc<MockConnector> = MockConnector::builder()
        .name("quota-mock")
        .returns_quote_ok(base_quote())
        .returns_profile_ok(base_profile())
        .returns_analyst_price_target_ok(base_price_target())
        .returns_recommendations_summary_ok(base_recommendation_summary())
        .build();
    let quota_cfg = quota_config(limit);
    ConnectorBuilder::new(raw as Arc<dyn BorsaConnector>)
        .with_quota(&quota_cfg)
        .build()
        .expect("quota builder should succeed")
}

fn blacklist_wrapped_connector() -> (
    Arc<dyn BorsaConnector>,
    Arc<AtomicBool>,
    Arc<AtomicBool>,
) {
    let fail_internal = Arc::new(AtomicBool::new(true));
    let fail_external = Arc::new(AtomicBool::new(false));

    let internal_flag = Arc::clone(&fail_internal);
    let external_flag = Arc::clone(&fail_external);

    let quote_fn = move |_inst: &Instrument| {
        if internal_flag.swap(false, Ordering::SeqCst) {
            return Err(BorsaError::RateLimitExceeded {
                limit: 1,
                window_ms: 50,
            });
        }
        if external_flag.swap(false, Ordering::SeqCst) {
            return Err(BorsaError::RateLimitExceeded {
                limit: 1,
                window_ms: 50,
            });
        }
        Ok(base_quote())
    };

    let raw: Arc<MockConnector> = MockConnector::builder()
        .name("blacklist-mock")
        .with_quote_fn(quote_fn)
        .returns_profile_ok(base_profile())
        .returns_analyst_price_target_ok(base_price_target())
        .returns_recommendations_summary_ok(base_recommendation_summary())
        .build();

    let wrapped = ConnectorBuilder::new(raw as Arc<dyn BorsaConnector>)
        .with_blacklist(Duration::from_millis(200))
        .build()
        .expect("blacklist builder should succeed");

    (wrapped, fail_internal, fail_external)
}

#[tokio::test]
async fn info_internal_calls_do_not_consume_quota() {
    let connector = quota_wrapped_connector(1);
    let borsa = Borsa::builder()
        .with_connector(Arc::clone(&connector))
        .build()
        .expect("borsa builder");
    let inst = instrument();

    // Internal fan-out should not burn the only quota unit.
    let report = borsa.info(&inst).await.expect("info should succeed");
    assert!(report.info.is_some(), "expected info payload");

    // First external quote consumes the unit.
    borsa.quote(&inst).await.expect("first quote should succeed");

    // Second external quote exceeds quota.
    let err = borsa.quote(&inst).await.expect_err("second quote should exceed quota");
    assert!(matches!(err, BorsaError::QuotaExceeded { .. }));

    // Despite quota exhaustion, info should still succeed because nested calls are internal.
    let report = borsa.info(&inst).await.expect("info should still succeed after quota is exhausted");
    assert!(report.info.is_some(), "expected info payload after quota exhaustion");
}

#[tokio::test]
async fn internal_rate_limit_does_not_trigger_blacklist() {
    let (connector, fail_internal, fail_external) = blacklist_wrapped_connector();
    let borsa = Borsa::builder()
        .with_connector(Arc::clone(&connector))
        .build()
        .expect("borsa builder");
    let inst = instrument();

    // Internal call hits provider rate limit but must not blacklist the connector.
    let report = borsa.info(&inst).await.expect("info should succeed despite internal rate limit");
    assert!(!report.info.is_none(), "info response expected");
    assert!(!fail_internal.load(Ordering::SeqCst), "internal flag should be consumed");

    // External quote succeeds (no blacklist in effect).
    borsa.quote(&inst).await.expect("first external quote should succeed");

    // Force next external call to simulate provider rate limit.
    fail_external.store(true, Ordering::SeqCst);
    let err = borsa.quote(&inst).await.expect_err("second external quote should propagate rate limit");
    assert!(matches!(err, BorsaError::RateLimitExceeded { .. }));

    // Subsequent external call should observe blacklist.
    let err = borsa.quote(&inst).await.expect_err("blacklist should reject subsequent quote");
    assert!(matches!(err, BorsaError::TemporarilyBlacklisted { .. }));

    // Internal fan-out should bypass blacklist.
    let report = borsa.info(&inst).await.expect("info should bypass blacklist");
    assert!(report.info.is_some(), "expected info despite blacklist");
}

