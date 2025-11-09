use crate::helpers::{AAPL, MSFT, m_hist};
use borsa::Borsa;
use borsa_core::{AssetKind, Instrument, Range, Symbol};

// A mock that can respond differently based on symbol
struct MultiSymbolHist;
#[async_trait::async_trait]
impl borsa_core::connector::HistoryProvider for MultiSymbolHist {
    async fn history(
        &self,
        i: &Instrument,
        r: borsa_core::HistoryRequest,
    ) -> Result<borsa_core::HistoryResponse, borsa_core::BorsaError> {
        let sym = match i.id() {
            borsa_core::IdentifierScheme::Security(sec) => sec.symbol.as_str(),
            borsa_core::IdentifierScheme::Prediction(_) => "",
        };
        match sym {
            "A" => m_hist("conn", &[1, 2]).history(i, r).await,
            "B" => m_hist("conn", &[10, 20]).history(i, r).await,
            "C" => m_hist("conn", &[100, 200]).history(i, r).await,
            _ => Err(borsa_core::BorsaError::not_found("unknown symbol")),
        }
    }

    fn supported_history_intervals(&self, _k: AssetKind) -> &'static [borsa_core::Interval] {
        crate::helpers::INTERVALS
    }
}
#[async_trait::async_trait]
impl borsa_core::BorsaConnector for MultiSymbolHist {
    fn name(&self) -> &'static str {
        "multi"
    }

    fn supports_kind(&self, _kind: AssetKind) -> bool {
        true
    }

    fn as_history_provider(&self) -> Option<&dyn borsa_core::connector::HistoryProvider> {
        Some(self as &dyn borsa_core::connector::HistoryProvider)
    }
}

#[tokio::test]
async fn download_builder_fetches_for_multiple_instruments() {
    let _mock_a = m_hist("conn", &[1, 2]);
    let _mock_b = m_hist("conn", &[10, 20]);
    let _mock_c = m_hist("conn", &[100, 200]);

    let borsa = Borsa::builder()
        .with_connector(std::sync::Arc::new(MultiSymbolHist))
        .build()
        .unwrap();

    let a = Symbol::new("A").expect("valid symbol");
    let b = Symbol::new("B").expect("valid symbol");
    let c = Symbol::new("C").expect("valid symbol");
    let instruments = &[
        crate::helpers::instrument(&a, AssetKind::Equity),
        crate::helpers::instrument(&b, AssetKind::Equity),
        crate::helpers::instrument(&c, AssetKind::Equity),
    ];

    let result = borsa
        .download()
        .instruments(instruments)
        .unwrap()
        .range(Range::D5)
        .run()
        .await
        .unwrap();

    let response = result.response.expect("download response");
    assert_eq!(response.entries.len(), 3);

    let entry_for = |sym: &str| {
        response
            .entries
            .iter()
            .find(|entry| match entry.instrument.id() {
                borsa_core::IdentifierScheme::Security(sec) => sec.symbol.as_str() == sym,
                borsa_core::IdentifierScheme::Prediction(_) => false,
            })
            .expect("entry for symbol")
    };

    assert_eq!(
        entry_for("A")
            .history
            .candles
            .iter()
            .map(|c| c.ts.timestamp())
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(
        entry_for("B")
            .history
            .candles
            .iter()
            .map(|c| c.ts.timestamp())
            .collect::<Vec<_>>(),
        vec![10, 20]
    );
    assert_eq!(
        entry_for("C")
            .history
            .candles
            .iter()
            .map(|c| c.ts.timestamp())
            .collect::<Vec<_>>(),
        vec![100, 200]
    );
}

#[tokio::test]
async fn download_builder_rejects_duplicate_symbols_in_instruments() {
    let borsa = Borsa::builder()
        .with_connector(std::sync::Arc::new(MultiSymbolHist))
        .build()
        .unwrap();

    let instruments_with_duplicates = &[
        crate::helpers::instrument(&AAPL, AssetKind::Equity),
        crate::helpers::instrument(&MSFT, AssetKind::Equity),
        crate::helpers::instrument(&AAPL, AssetKind::Equity), // Duplicate!
    ];

    let result = borsa.download().instruments(instruments_with_duplicates);

    match result {
        Ok(_) => panic!("Expected error for duplicate symbols"),
        Err(error) => {
            assert!(error.to_string().contains("duplicate symbol 'AAPL'"));
        }
    }
}

#[tokio::test]
async fn download_builder_rejects_duplicate_symbols_in_add_instrument() {
    let borsa = Borsa::builder()
        .with_connector(std::sync::Arc::new(MultiSymbolHist))
        .build()
        .unwrap();

    let aapl = Symbol::new("AAPL").expect("valid symbol");
    let result = borsa
        .download()
        .instruments(&[crate::helpers::instrument(&aapl, AssetKind::Equity)])
        .unwrap()
        .add_instrument(crate::helpers::instrument(&aapl, AssetKind::Equity));

    match result {
        Ok(_) => panic!("Expected error for duplicate symbol in add_instrument"),
        Err(error) => {
            assert!(
                error
                    .to_string()
                    .contains("duplicate symbol 'AAPL' already exists")
            );
        }
    }
}

#[tokio::test]
async fn download_builder_allows_different_symbols() {
    let borsa = Borsa::builder()
        .with_connector(std::sync::Arc::new(MultiSymbolHist))
        .build()
        .unwrap();

    let a = Symbol::new("A").expect("valid symbol");
    let b = Symbol::new("B").expect("valid symbol");
    let result = borsa
        .download()
        .instruments(&[crate::helpers::instrument(&a, AssetKind::Equity)])
        .unwrap()
        .add_instrument(crate::helpers::instrument(&b, AssetKind::Equity))
        .unwrap()
        .range(Range::D5)
        .run()
        .await;

    assert!(result.is_ok());
    let report = result.unwrap();
    let response = report.response.expect("download response");
    assert!(response.entries.len() >= 2);
}

#[tokio::test]
async fn download_builder_handles_empty_instruments_list() {
    let borsa = Borsa::builder()
        .with_connector(std::sync::Arc::new(MultiSymbolHist))
        .build()
        .unwrap();

    let result = borsa
        .download()
        .instruments(&[])
        .unwrap()
        .range(Range::D5)
        .run()
        .await;

    match result {
        Ok(_) => panic!("Expected error for empty instruments list"),
        Err(error) => {
            assert!(error.to_string().contains("no instruments specified"));
        }
    }
}
