use async_trait::async_trait;
use borsa::{Borsa, MergeStrategy};
use borsa_core::{
    AssetKind, BorsaError, Candle, Currency, HistoryRequest, HistoryResponse, Instrument, Money,
    connector::{BorsaConnector, HistoryProvider},
};
use chrono::TimeZone;
use std::sync::Arc;

/// A mock connector that simulates a fast, limited data provider.
/// It provides data for only the first few days of a request.
struct FastLimitedConnector;

#[async_trait]
impl BorsaConnector for FastLimitedConnector {
    fn name(&self) -> &'static str {
        "fast-limited"
    }

    fn as_history_provider(&self) -> Option<&dyn HistoryProvider> {
        Some(self as &dyn HistoryProvider)
    }
}

#[async_trait]
impl borsa_core::connector::HistoryProvider for FastLimitedConnector {
    #[allow(clippy::too_many_lines)]
    async fn history(
        &self,
        _i: &Instrument,
        _r: HistoryRequest,
    ) -> Result<HistoryResponse, BorsaError> {
        // Simulate a fast provider that only has data for the first 3 days
        let candles = vec![
            Candle {
                ts: chrono::Utc.timestamp_opt(1_640_995_200, 0).unwrap(),
                open: Money::from_canonical_str(
                    "100.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                high: Money::from_canonical_str(
                    "105.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                low: Money::from_canonical_str("98.0", Currency::Iso(borsa_core::IsoCurrency::USD))
                    .unwrap(),
                close: Money::from_canonical_str(
                    "102.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                close_unadj: None,
                volume: Some(1_000_000),
            },
            Candle {
                ts: chrono::Utc.timestamp_opt(1_641_081_600, 0).unwrap(),
                open: Money::from_canonical_str(
                    "102.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                high: Money::from_canonical_str(
                    "108.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                low: Money::from_canonical_str(
                    "101.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                close: Money::from_canonical_str(
                    "106.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                close_unadj: None,
                volume: Some(1_200_000),
            },
            Candle {
                ts: chrono::Utc.timestamp_opt(1_641_168_000, 0).unwrap(),
                open: Money::from_canonical_str(
                    "106.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                high: Money::from_canonical_str(
                    "110.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                low: Money::from_canonical_str(
                    "104.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                close: Money::from_canonical_str(
                    "108.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                close_unadj: None,
                volume: Some(1_100_000),
            },
        ];

        Ok(HistoryResponse {
            candles,
            actions: vec![],
            adjusted: false,
            meta: None,
        })
    }

    fn supported_history_intervals(&self, _i: AssetKind) -> &'static [borsa_core::Interval] {
        &[borsa_core::Interval::D1]
    }
}

/// A mock connector that simulates a slower, comprehensive data provider.
/// It provides data for a longer period but takes more time.
struct SlowComprehensiveConnector;

#[async_trait]
impl BorsaConnector for SlowComprehensiveConnector {
    fn name(&self) -> &'static str {
        "slow-comprehensive"
    }

    fn as_history_provider(&self) -> Option<&dyn HistoryProvider> {
        Some(self as &dyn HistoryProvider)
    }
}

#[async_trait]
impl HistoryProvider for SlowComprehensiveConnector {
    #[allow(clippy::too_many_lines)]
    async fn history(
        &self,
        _i: &Instrument,
        _r: HistoryRequest,
    ) -> Result<HistoryResponse, BorsaError> {
        // Simulate a slower provider that has comprehensive data
        let candles = vec![
            Candle {
                ts: chrono::Utc.timestamp_opt(1_640_995_200, 0).unwrap(),
                open: Money::from_canonical_str(
                    "100.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                high: Money::from_canonical_str(
                    "105.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                low: Money::from_canonical_str("98.0", Currency::Iso(borsa_core::IsoCurrency::USD))
                    .unwrap(),
                close: Money::from_canonical_str(
                    "102.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                close_unadj: None,
                volume: Some(1_000_000),
            },
            Candle {
                ts: chrono::Utc.timestamp_opt(1_641_081_600, 0).unwrap(),
                open: Money::from_canonical_str(
                    "102.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                high: Money::from_canonical_str(
                    "108.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                low: Money::from_canonical_str(
                    "101.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                close: Money::from_canonical_str(
                    "106.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                close_unadj: None,
                volume: Some(1_200_000),
            },
            Candle {
                ts: chrono::Utc.timestamp_opt(1_641_168_000, 0).unwrap(),
                open: Money::from_canonical_str(
                    "106.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                high: Money::from_canonical_str(
                    "110.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                low: Money::from_canonical_str(
                    "104.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                close: Money::from_canonical_str(
                    "108.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                close_unadj: None,
                volume: Some(1_100_000),
            },
            Candle {
                ts: chrono::Utc.timestamp_opt(1_641_254_400, 0).unwrap(),
                open: Money::from_canonical_str(
                    "108.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                high: Money::from_canonical_str(
                    "112.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                low: Money::from_canonical_str(
                    "106.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                close: Money::from_canonical_str(
                    "110.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                close_unadj: None,
                volume: Some(1_300_000),
            },
            Candle {
                ts: chrono::Utc.timestamp_opt(1_641_340_800, 0).unwrap(),
                open: Money::from_canonical_str(
                    "110.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                high: Money::from_canonical_str(
                    "115.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                low: Money::from_canonical_str(
                    "109.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                close: Money::from_canonical_str(
                    "113.0",
                    Currency::Iso(borsa_core::IsoCurrency::USD),
                )
                .unwrap(),
                close_unadj: None,
                volume: Some(1_400_000),
            },
        ];

        Ok(HistoryResponse {
            candles,
            actions: vec![],
            adjusted: false,
            meta: None,
        })
    }

    fn supported_history_intervals(&self, _i: AssetKind) -> &'static [borsa_core::Interval] {
        &[borsa_core::Interval::D1]
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔄 Borsa Merge Strategies Demo");
    println!("==============================\n");

    // Create our mock connectors
    let fast_connector = Arc::new(FastLimitedConnector);
    let slow_connector = Arc::new(SlowComprehensiveConnector);

    let instrument =
        Instrument::from_symbol("DEMO", AssetKind::Equity).expect("valid instrument symbol");
    let req =
        HistoryRequest::try_from_range(borsa_core::Range::D1, borsa_core::Interval::D1).unwrap();

    // 1. Demonstrate Deep Strategy (default behavior)
    println!("📊 Deep Strategy (Default)");
    println!("--------------------------");
    println!("Fetches from ALL providers and merges their data for maximum completeness.\n");

    let borsa_deep = Borsa::builder()
        .with_connector(fast_connector.clone())
        .with_connector(slow_connector.clone())
        .merge_history_strategy(MergeStrategy::Deep)
        .build();

    let (history_deep, attribution_deep) = borsa_deep
        .history_with_attribution(&instrument, req.clone())
        .await?;

    println!("📈 Deep Strategy Results:");
    println!("  - Total candles: {}", history_deep.candles.len());
    println!(
        "  - Date range: {} to {}",
        history_deep.candles.first().unwrap().ts.timestamp(),
        history_deep.candles.last().unwrap().ts.timestamp()
    );
    println!("  - Providers used: {}", attribution_deep.spans.len());
    for (provider, span) in &attribution_deep.spans {
        println!(
            "    * {}: {} candles (ts {} to {})",
            provider,
            (span.end - span.start) + 1,
            span.start,
            span.end
        );
    }
    println!();

    // 2. Demonstrate Fallback Strategy
    println!("⚡ Fallback Strategy");
    println!("-------------------");
    println!("Stops at the FIRST provider with data to minimize API calls.\n");

    let borsa_fallback = Borsa::builder()
        .with_connector(fast_connector.clone())
        .with_connector(slow_connector.clone())
        .merge_history_strategy(MergeStrategy::Fallback)
        .build();

    let (history_fallback, attribution_fallback) = borsa_fallback
        .history_with_attribution(&instrument, req)
        .await?;

    println!("📈 Fallback Strategy Results:");
    println!("  - Total candles: {}", history_fallback.candles.len());
    println!(
        "  - Date range: {} to {}",
        history_fallback.candles.first().unwrap().ts.timestamp(),
        history_fallback.candles.last().unwrap().ts.timestamp()
    );
    println!("  - Providers used: {}", attribution_fallback.spans.len());
    for (provider, span) in &attribution_fallback.spans {
        println!(
            "    * {}: {} candles (ts {} to {})",
            provider,
            (span.end - span.start) + 1,
            span.start,
            span.end
        );
    }
    println!();

    // 3. Compare the strategies
    println!("🔍 Strategy Comparison");
    println!("---------------------");
    println!("Deep Strategy:");
    println!(
        "  ✅ Most complete dataset ({} candles)",
        history_deep.candles.len()
    );
    println!(
        "  ❌ Uses more API calls ({} providers)",
        attribution_deep.spans.len()
    );
    println!("  💡 Best for: Research, backtesting, when data completeness is critical");
    println!();
    println!("Fallback Strategy:");
    println!(
        "  ✅ Economical API usage ({} providers)",
        attribution_fallback.spans.len()
    );
    println!(
        "  ❌ May miss data from lower-priority providers ({} candles)",
        history_fallback.candles.len()
    );
    println!("  💡 Best for: Production apps with API rate limits, when speed matters");
    println!();

    println!("🎯 Choose your strategy based on your needs:");
    println!("  - Use Deep for maximum data completeness");
    println!("  - Use Fallback for API rate limit efficiency");

    Ok(())
}
