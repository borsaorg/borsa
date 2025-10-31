use borsa::Borsa;

use crate::helpers::{MockConnector, X};
use borsa_core::{AssetKind, BorsaError, HistoryRequest};

#[tokio::test]
async fn all_not_found_returns_not_found() {
    const SUPP: &[borsa_core::Interval] = &[
        borsa_core::Interval::I1m,
        borsa_core::Interval::I2m,
        borsa_core::Interval::I15m,
        borsa_core::Interval::I30m,
        borsa_core::Interval::I90m,
        borsa_core::Interval::D1,
        borsa_core::Interval::W1,
    ];
    let nf = MockConnector::builder()
        .name("nf_hist")
        .with_history_intervals(SUPP)
        .with_history_fn(|_i, _r| Err(BorsaError::not_found("history for X")))
        .build();
    let borsa = Borsa::builder().with_connector(nf).build().unwrap();

    let inst = crate::helpers::instrument(&X, AssetKind::Equity);
    let err = borsa
        .history(
            &inst,
            HistoryRequest::try_from_range(borsa_core::Range::D1, borsa_core::Interval::D1)
                .unwrap(),
        )
        .await
        .expect_err("should error");
    assert!(matches!(err, BorsaError::NotFound { .. }));
}

#[tokio::test]
async fn all_ok_empty_returns_not_found() {
    const SUPP: &[borsa_core::Interval] = &[
        borsa_core::Interval::I1m,
        borsa_core::Interval::I2m,
        borsa_core::Interval::I15m,
        borsa_core::Interval::I30m,
        borsa_core::Interval::I90m,
        borsa_core::Interval::D1,
        borsa_core::Interval::W1,
    ];

    let empty_ok = MockConnector::builder()
        .name("empty_ok")
        .with_history_intervals(SUPP)
        .returns_history_ok(borsa_core::HistoryResponse {
            candles: vec![],
            actions: vec![],
            adjusted: false,
            meta: None,
        })
        .build();

    let borsa = Borsa::builder().with_connector(empty_ok).build().unwrap();

    let inst = crate::helpers::instrument(&X, AssetKind::Equity);
    let err = borsa
        .history(
            &inst,
            HistoryRequest::try_from_range(borsa_core::Range::D1, borsa_core::Interval::D1)
                .unwrap(),
        )
        .await
        .expect_err("should error");
    assert!(matches!(err, BorsaError::NotFound { .. }));
}
