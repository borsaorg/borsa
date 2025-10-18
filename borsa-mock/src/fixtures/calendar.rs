use borsa_core::Calendar;
use chrono::TimeZone;

pub fn by_symbol(_s: &str) -> Option<Calendar> {
    Some(Calendar {
        earnings_dates: vec![chrono::Utc.with_ymd_and_hms(2023, 2, 1, 0, 0, 0).unwrap()],
        ex_dividend_date: None,
        dividend_payment_date: None,
    })
}
