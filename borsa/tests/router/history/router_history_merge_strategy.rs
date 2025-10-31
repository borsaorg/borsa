use crate::helpers::{MockConnector, m_hist};
use borsa::{Borsa, MergeStrategy};
use borsa_core::{AssetKind, HistoryRequest, Interval, Range, Symbol};

#[tokio::test]
async fn merge_strategy_deep_fetches_all_providers() {
    // Create three mock connectors with different data
    let a = m_hist("A", &[1, 2, 3]); // First provider has data for timestamps 1-3
    let b = m_hist("B", &[4, 5, 6]); // Second provider has data for timestamps 4-6
    let c = m_hist("C", &[7, 8, 9]); // Third provider has data for timestamps 7-9

    let borsa = Borsa::builder()
        .with_connector(a)
        .with_connector(b)
        .with_connector(c)
        .merge_history_strategy(MergeStrategy::Deep)
        .build()
        .unwrap();

    let test = Symbol::new("TEST").expect("valid symbol");
    let inst = crate::helpers::instrument(&test, AssetKind::Equity);
    let req = HistoryRequest::try_from_range(Range::D1, Interval::D1).unwrap();

    let (merged, attr) = borsa.history_with_attribution(&inst, req).await.unwrap();

    // With Deep strategy, all providers should be queried and data merged
    let ts: Vec<i64> = merged.candles.iter().map(|c| c.ts.timestamp()).collect();
    assert_eq!(ts, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);

    // All three providers should have contributed data
    assert_eq!(attr.spans.len(), 3);

    // Check that all providers are represented in attribution
    let provider_names: Vec<&str> = attr.spans.iter().map(|(name, _)| *name).collect();
    assert!(provider_names.contains(&"A"));
    assert!(provider_names.contains(&"B"));
    assert!(provider_names.contains(&"C"));
}

#[tokio::test]
async fn merge_strategy_fallback_stops_at_first_success() {
    // Create three mock connectors, but only the first one will have data
    let a = m_hist("A", &[1, 2, 3]); // First provider has data
    let b = MockConnector::builder().name("B").build();
    let c = m_hist("C", &[7, 8, 9]); // Third provider has data but shouldn't be reached

    let borsa = Borsa::builder()
        .with_connector(a)
        .with_connector(b)
        .with_connector(c)
        .merge_history_strategy(MergeStrategy::Fallback)
        .build()
        .unwrap();

    let test = Symbol::new("TEST").expect("valid symbol");
    let inst = crate::helpers::instrument(&test, AssetKind::Equity);
    let req = HistoryRequest::try_from_range(Range::D1, Interval::D1).unwrap();

    let (merged, attr) = borsa.history_with_attribution(&inst, req).await.unwrap();

    // With Fallback strategy, only the first provider with data should be used
    let ts: Vec<i64> = merged.candles.iter().map(|c| c.ts.timestamp()).collect();
    assert_eq!(ts, vec![1, 2, 3]);

    // Only the first provider should have contributed data
    assert_eq!(attr.spans.len(), 1);
    assert_eq!(attr.spans[0].0, "A");
}

#[tokio::test]
async fn merge_strategy_fallback_continues_on_empty_data() {
    // Create connectors where first returns empty data, second has actual data
    let a = MockConnector::builder()
        .name("A")
        .returns_history_ok(borsa_core::HistoryResponse {
            candles: vec![], // Empty data - should cause fallback to continue
            actions: vec![],
            adjusted: false,
            meta: None,
        })
        .build();
    let b = m_hist("B", &[4, 5, 6]); // Second provider has data
    let c = m_hist("C", &[7, 8, 9]); // Third provider has data but shouldn't be reached

    let borsa = Borsa::builder()
        .with_connector(a)
        .with_connector(b)
        .with_connector(c)
        .merge_history_strategy(MergeStrategy::Fallback)
        .build()
        .unwrap();
    
    let test = Symbol::new("TEST").expect("valid symbol");
    let inst = crate::helpers::instrument(&test, AssetKind::Equity);
    let req = HistoryRequest::try_from_range(Range::D1, Interval::D1).unwrap();

    let (merged, attr) = borsa.history_with_attribution(&inst, req).await.unwrap();

    // With Fallback strategy, should use the second provider (first with actual data)
    let ts: Vec<i64> = merged.candles.iter().map(|c| c.ts.timestamp()).collect();
    assert_eq!(ts, vec![4, 5, 6]);

    // Only the second provider should have contributed data
    assert_eq!(attr.spans.len(), 1);
    assert_eq!(attr.spans[0].0, "B");
}

#[tokio::test]
async fn merge_strategy_fallback_handles_errors_gracefully() {
    // Create connectors where first fails, second succeeds
    let a = MockConnector::builder().name("A").build();
    let b = m_hist("B", &[4, 5, 6]); // Second provider has data
    let c = m_hist("C", &[7, 8, 9]); // Third provider has data but shouldn't be reached

    let borsa = Borsa::builder()
        .with_connector(a)
        .with_connector(b)
        .with_connector(c)
        .merge_history_strategy(MergeStrategy::Fallback)
        .build()
        .unwrap();

    let test = Symbol::new("TEST").expect("valid symbol");
    let inst = crate::helpers::instrument(&test, AssetKind::Equity);
    let req = HistoryRequest::try_from_range(Range::D1, Interval::D1).unwrap();

    let (merged, attr) = borsa.history_with_attribution(&inst, req).await.unwrap();

    // With Fallback strategy, should use the second provider after first fails
    let ts: Vec<i64> = merged.candles.iter().map(|c| c.ts.timestamp()).collect();
    assert_eq!(ts, vec![4, 5, 6]);

    // Only the second provider should have contributed data
    assert_eq!(attr.spans.len(), 1);
    assert_eq!(attr.spans[0].0, "B");
}

#[tokio::test]
async fn merge_strategy_default_is_deep() {
    // Test that the default strategy is Deep (backward compatibility)
    let a = m_hist("A", &[1, 2, 3]);
    let b = m_hist("B", &[4, 5, 6]);

    let borsa = Borsa::builder()
        .with_connector(a)
        .with_connector(b)
        .build()
        .unwrap(); // No explicit strategy set

    let test = Symbol::new("TEST").expect("valid symbol");
    let inst = crate::helpers::instrument(&test, AssetKind::Equity);
    let req = HistoryRequest::try_from_range(Range::D1, Interval::D1).unwrap();

    let (merged, attr) = borsa.history_with_attribution(&inst, req).await.unwrap();

    // Default behavior should be Deep (merge all providers)
    let ts: Vec<i64> = merged.candles.iter().map(|c| c.ts.timestamp()).collect();
    assert_eq!(ts, vec![1, 2, 3, 4, 5, 6]);

    // Both providers should have contributed data
    assert_eq!(attr.spans.len(), 2);
}

#[tokio::test]
async fn merge_strategy_fallback_with_overlapping_data() {
    // Test fallback behavior when providers have overlapping data
    // First provider should win for overlapping timestamps
    let a = m_hist("A", &[1, 2, 3, 4]);
    let b = m_hist("B", &[3, 4, 5, 6]); // Overlaps with A at timestamps 3,4

    let borsa = Borsa::builder()
        .with_connector(a)
        .with_connector(b)
        .merge_history_strategy(MergeStrategy::Fallback)
        .build()
        .unwrap();

    let test = Symbol::new("TEST").expect("valid symbol");
    let inst = crate::helpers::instrument(&test, AssetKind::Equity);
    let req = HistoryRequest::try_from_range(Range::D1, Interval::D1).unwrap();

    let (merged, attr) = borsa.history_with_attribution(&inst, req).await.unwrap();

    // With Fallback strategy, only first provider should be used
    let ts: Vec<i64> = merged.candles.iter().map(|c| c.ts.timestamp()).collect();
    assert_eq!(ts, vec![1, 2, 3, 4]);

    // Only the first provider should have contributed data
    assert_eq!(attr.spans.len(), 1);
    assert_eq!(attr.spans[0].0, "A");
}
