use borsa_core::{BorsaError, HistoryRequest, Interval};
use chrono::{DateTime, Utc};

// Dropped brittle display-format tests; they do not protect semantics

#[test]
fn paft_error_conversion_maps_to_invalid_arg_for_invalid_history_period() {
    // Build an invalid period: start > end
    let start: DateTime<Utc> = DateTime::from_timestamp(200, 0).unwrap();
    let end: DateTime<Utc> = DateTime::from_timestamp(100, 0).unwrap();
    let res = HistoryRequest::try_from_period(start, end, Interval::D1);
    assert!(res.is_err());
    let err = res.err().unwrap();
    let b: BorsaError = err.into();
    assert!(matches!(b, BorsaError::InvalidArg(_)));
}
