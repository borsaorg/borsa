//! Report envelopes produced by orchestrators and helpers.

use paft::aggregates::Info;
use paft::domain::Symbol;
use paft::market::responses::download::DownloadResponse;
use paft::market::responses::search::SearchResponse;
use serde::{Deserialize, Serialize};

use crate::error::BorsaError;

/// Summary of instrument information retrieval.
///
/// Carries the requested `symbol`, the resolved [`Info`] snapshot if
/// available, and any non-fatal warnings encountered during processing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct InfoReport {
    /// Requested symbol.
    pub symbol: Symbol,
    /// Snapshot payload, if successfully resolved.
    pub info: Option<Info>,
    /// Non-fatal issues encountered while building the report.
    pub warnings: Vec<BorsaError>,
}

/// Summary of a symbol search operation.
///
/// Contains the upstream search `response` when present and any associated
/// `warnings`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SearchReport {
    /// Upstream search response payload.
    pub response: Option<SearchResponse>,
    /// Non-fatal issues encountered while building the report.
    pub warnings: Vec<BorsaError>,
}

/// Summary of historical data download.
///
/// Wraps a [`DownloadResponse`] payload when present and any `warnings`
/// captured during retrieval or normalization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DownloadReport {
    /// Aggregated download payload.
    pub response: Option<DownloadResponse>,
    /// Non-fatal issues encountered while building the report.
    pub warnings: Vec<BorsaError>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BorsaError;
    use chrono::{TimeZone, Utc};
    use paft::aggregates::Info;
    use paft::domain::{AssetKind, Exchange, Instrument};
    use paft::market::responses::download::{DownloadEntry, DownloadResponse};
    use paft::market::responses::history::{Candle, HistoryMeta, HistoryResponse};
    use paft::market::responses::search::{SearchResponse, SearchResult};
    use paft::money::{Currency, IsoCurrency, Money};

    #[test]
    fn info_report_roundtrip() {
        let report = InfoReport {
            symbol: paft::domain::Symbol::new("AAPL").unwrap(),
            info: Some(Info {
                symbol: paft::domain::Symbol::new("AAPL").unwrap(),
                name: Some("Apple Inc.".into()),
                isin: None,
                exchange: Some(Exchange::NASDAQ),
                market_state: None,
                currency: Some(Currency::Iso(IsoCurrency::USD)),
                last: None,
                open: None,
                high: None,
                low: None,
                previous_close: None,
                day_range_low: None,
                day_range_high: None,
                fifty_two_week_low: None,
                fifty_two_week_high: None,
                volume: None,
                average_volume: None,
                market_cap: None,
                shares_outstanding: None,
                eps_ttm: None,
                pe_ttm: None,
                dividend_yield: None,
                ex_dividend_date: None,
                as_of: None,
            }),
            warnings: vec![BorsaError::Data("data stale".into())],
        };

        let json = serde_json::to_string(&report).unwrap();
        let back: InfoReport = serde_json::from_str(&json).unwrap();
        assert_eq!(back, report);
    }

    #[test]
    fn search_report_roundtrip() {
        let report = SearchReport {
            response: Some(SearchResponse {
                results: vec![SearchResult {
                    symbol: paft::domain::Symbol::new("AAPL").unwrap(),
                    name: Some("Apple Inc.".into()),
                    exchange: Some(Exchange::NASDAQ),
                    kind: AssetKind::Equity,
                }],
            }),
            warnings: vec![BorsaError::Data("partial response".into())],
        };

        let json = serde_json::to_string(&report).unwrap();
        let back: SearchReport = serde_json::from_str(&json).unwrap();
        assert_eq!(back, report);
    }

    #[test]
    fn download_report_roundtrip() {
        let usd = Currency::Iso(IsoCurrency::USD);
        let base_ts = Utc.timestamp_opt(1_700_000_000, 0).unwrap();

        let candle = Candle {
            ts: base_ts,
            open: Money::from_canonical_str("1.00", usd.clone()).unwrap(),
            high: Money::from_canonical_str("2.00", usd.clone()).unwrap(),
            low: Money::from_canonical_str("0.50", usd.clone()).unwrap(),
            close: Money::from_canonical_str("1.50", usd).unwrap(),
            close_unadj: None,
            volume: Some(1000),
        };

        let payload = HistoryResponse {
            candles: vec![candle],
            actions: vec![],
            adjusted: true,
            meta: Some(HistoryMeta {
                timezone: Some(chrono_tz::America::New_York),
                utc_offset_seconds: Some(-18_000),
            }),
        };

        let entries = vec![DownloadEntry {
            instrument: Instrument::from_symbol("AAPL", AssetKind::Equity).unwrap(),
            history: payload,
        }];

        let report = DownloadReport {
            response: Some(DownloadResponse { entries }),
            warnings: vec![BorsaError::Data("fallback provider used".into())],
        };

        let json = serde_json::to_string(&report).unwrap();
        let back: DownloadReport = serde_json::from_str(&json).unwrap();
        assert_eq!(back, report);
    }

    #[test]
    fn download_report_dual_listed_roundtrip() {
        let usd = Currency::Iso(IsoCurrency::USD);
        let base_ts = Utc.timestamp_opt(1_700_000_000, 0).unwrap();

        let candle = Candle {
            ts: base_ts,
            open: Money::from_canonical_str("1.00", usd.clone()).unwrap(),
            high: Money::from_canonical_str("2.00", usd.clone()).unwrap(),
            low: Money::from_canonical_str("0.50", usd.clone()).unwrap(),
            close: Money::from_canonical_str("1.50", usd).unwrap(),
            close_unadj: None,
            volume: Some(1000),
        };

        let payload = HistoryResponse {
            candles: vec![candle],
            actions: vec![],
            adjusted: true,
            meta: None,
        };

        let entries = vec![
            DownloadEntry {
                instrument: Instrument::from_symbol_and_exchange(
                    "AAPL",
                    Exchange::NASDAQ,
                    AssetKind::Equity,
                )
                .unwrap(),
                history: payload.clone(),
            },
            DownloadEntry {
                instrument: Instrument::from_symbol_and_exchange(
                    "AAPL",
                    Exchange::LSE,
                    AssetKind::Equity,
                )
                .unwrap(),
                history: payload,
            },
        ];

        let report = DownloadReport {
            response: Some(DownloadResponse { entries }),
            warnings: vec![],
        };

        let json = serde_json::to_string(&report).unwrap();
        let back: DownloadReport = serde_json::from_str(&json).unwrap();
        assert_eq!(back, report);
    }
}
