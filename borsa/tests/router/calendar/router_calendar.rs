use crate::helpers::MockConnector;
use borsa::Borsa;
use borsa_core::{AssetKind, BorsaError, Calendar};
// use chrono::TimeZone;
use crate::helpers::dt;

#[tokio::test]
async fn calendar_falls_back_and_succeeds() {
    let err_cal = MockConnector::builder()
        .name("err_calendar")
        .with_calendar_fn(|_| Err(BorsaError::unsupported("calendar")))
        .build();

    let ok_calendar = Calendar {
        earnings_dates: vec![dt(2024, 6, 18, 0, 0, 0), dt(2025, 8, 24, 0, 0, 0)],
        ex_dividend_date: None,
        dividend_payment_date: None,
    };

    let ok_cal = MockConnector::builder()
        .name("ok_calendar")
        .returns_calendar_ok(ok_calendar)
        .build();

    let borsa = Borsa::builder()
        .with_connector(err_cal)
        .with_connector(ok_cal)
        .build();

    let inst = crate::helpers::instrument("TSLA", AssetKind::Equity);
    let cal = borsa.calendar(&inst).await.unwrap();

    assert_eq!(
        cal.earnings_dates
            .iter()
            .map(chrono::DateTime::timestamp)
            .collect::<Vec<_>>(),
        vec![
            dt(2024, 6, 18, 0, 0, 0).timestamp(),
            dt(2025, 8, 24, 0, 0, 0).timestamp()
        ]
    );
    assert!(cal.ex_dividend_date.is_none());
    assert!(cal.dividend_payment_date.is_none());
}
