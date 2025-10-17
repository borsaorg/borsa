use std::collections::HashMap;
use std::sync::Arc;

use crate::helpers::usd;
use crate::helpers::{AAPL, GOOG, MSFT};
use async_trait::async_trait;
use borsa::Borsa;
use borsa_core::{AssetKind, BorsaConnector, BorsaError, Instrument, Quote};
use rust_decimal::Decimal;

/// Connector that returns quotes only for symbols present in `ok_prices`.
/// Missing symbols return `NotFound`.
struct MapConnector {
    name: &'static str,
    ok_prices: HashMap<String, f64>,
}

#[async_trait]
impl borsa_core::connector::QuoteProvider for MapConnector {
    async fn quote(&self, inst: &Instrument) -> Result<Quote, BorsaError> {
        if let Some(&p) = self.ok_prices.get(inst.symbol_str()) {
            Ok(Quote {
                symbol: inst.symbol().clone(),
                shortname: None,
                price: Some(usd(&p.to_string())),
                previous_close: None,
                exchange: None,
                market_state: None,
            })
        } else {
            Err(BorsaError::not_found(format!(
                "quote for {}",
                inst.symbol()
            )))
        }
    }
}

#[async_trait]
impl BorsaConnector for MapConnector {
    fn name(&self) -> &'static str {
        self.name
    }

    fn supports_kind(&self, _k: AssetKind) -> bool {
        true
    }
    fn as_quote_provider(&self) -> Option<&dyn borsa_core::connector::QuoteProvider> {
        Some(self)
    }
}

#[tokio::test]
async fn quotes_per_symbol_fallback_succeeds() {
    // Top provider serves AAPL/MSFT but not GOOG.
    let mut top_map = HashMap::new();
    top_map.insert(AAPL.into(), 100.0);
    top_map.insert(MSFT.into(), 200.0);
    let top = Arc::new(MapConnector {
        name: "top",
        ok_prices: top_map,
    });

    // Backup provider serves GOOG.
    let mut backup_map = HashMap::new();
    backup_map.insert(GOOG.into(), 300.0);
    let backup = Arc::new(MapConnector {
        name: "backup",
        ok_prices: backup_map,
    });

    // Build router preferring top over backup.
    let borsa = Borsa::builder()
        .with_connector(top)
        .with_connector(backup)
        .build()
        .unwrap();

    let insts = &[
        crate::helpers::instrument(AAPL, AssetKind::Equity),
        crate::helpers::instrument(MSFT, AssetKind::Equity),
        crate::helpers::instrument(GOOG, AssetKind::Equity),
    ];

    let (out, errs) = borsa.quotes(insts).await.expect("quotes ok");
    assert!(errs.is_empty());

    // We should get all three quotes: AAPL/MSFT from top, GOOG from backup.
    assert_eq!(out.len(), 3);

    let by_symbol: std::collections::HashMap<_, _> =
        out.iter().map(|q| (q.symbol.as_str(), q)).collect();
    assert_eq!(
        by_symbol
            .get(AAPL)
            .unwrap()
            .price
            .as_ref()
            .unwrap()
            .amount(),
        Decimal::from(100u8)
    );
    assert_eq!(
        by_symbol
            .get(MSFT)
            .unwrap()
            .price
            .as_ref()
            .unwrap()
            .amount(),
        Decimal::from(200u8)
    );
    assert_eq!(
        by_symbol
            .get(GOOG)
            .unwrap()
            .price
            .as_ref()
            .unwrap()
            .amount(),
        Decimal::from(300u16)
    );
}
