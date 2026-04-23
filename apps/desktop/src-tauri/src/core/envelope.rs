// Envelope budget logic — T-013, T-005, T-048

use chrono::{Datelike, NaiveDate, TimeZone};
use chrono_tz::Tz;

/// Returns (period_start_ms, period_end_ms) for the local calendar month
/// containing `now_ms`, expressed in the household's IANA timezone `tz`.
///
/// Both returned values are unix-milliseconds of the UTC instant that
/// corresponds to **midnight local time** on the first and last days of
/// the month respectively — the same convention used by `transactions.txn_date`.
///
/// Returns `Err` if `tz` is not a valid IANA zone name.
pub fn current_month_bounds_ms(tz: &str, now_ms: i64) -> Result<(i64, i64), String> {
    let tz: Tz = tz.parse().map_err(|e| format!("Invalid timezone '{tz}': {e}"))?;
    let now_utc = chrono::DateTime::from_timestamp_millis(now_ms)
        .ok_or_else(|| format!("Invalid unix-ms timestamp: {now_ms}"))?;
    let now_local = now_utc.with_timezone(&tz);

    let year = now_local.year();
    let month = now_local.month();

    let first = NaiveDate::from_ymd_opt(year, month, 1)
        .ok_or_else(|| format!("Invalid year/month: {year}/{month}"))?;
    let (next_year, next_month) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    let next_first = NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .expect("next month always valid");
    let last = next_first
        .pred_opt()
        .expect("month always has at least one day");

    let start = tz
        .from_local_datetime(&first.and_hms_opt(0, 0, 0).unwrap())
        .single()
        .ok_or_else(|| "Ambiguous local midnight at period start".to_string())?;
    let end = tz
        .from_local_datetime(&last.and_hms_opt(0, 0, 0).unwrap())
        .single()
        .ok_or_else(|| "Ambiguous local midnight at period end".to_string())?;

    Ok((start.timestamp_millis(), end.timestamp_millis()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use chrono_tz::Tz;

    fn ms_from_ymd(tz: Tz, y: i32, m: u32, d: u32) -> i64 {
        tz.with_ymd_and_hms(y, m, d, 0, 0, 0).unwrap().timestamp_millis()
    }

    #[test]
    fn month_bounds_utc_january() {
        let now = ms_from_ymd(chrono_tz::UTC, 2026, 1, 15);
        let (start, end) = current_month_bounds_ms("UTC", now).unwrap();
        assert_eq!(start, ms_from_ymd(chrono_tz::UTC, 2026, 1, 1));
        assert_eq!(end, ms_from_ymd(chrono_tz::UTC, 2026, 1, 31));
    }

    #[test]
    fn month_bounds_utc_december_crosses_year() {
        let now = ms_from_ymd(chrono_tz::UTC, 2025, 12, 20);
        let (start, end) = current_month_bounds_ms("UTC", now).unwrap();
        assert_eq!(start, ms_from_ymd(chrono_tz::UTC, 2025, 12, 1));
        assert_eq!(end, ms_from_ymd(chrono_tz::UTC, 2025, 12, 31));
    }

    #[test]
    fn month_bounds_chicago_differs_from_utc() {
        // 2026-03-15T05:30:00Z is 2026-03-15 00:30 in Chicago.
        let now = chrono::Utc
            .with_ymd_and_hms(2026, 3, 15, 5, 30, 0)
            .unwrap()
            .timestamp_millis();
        let chi: Tz = "America/Chicago".parse().unwrap();
        let (start, _end) = current_month_bounds_ms("America/Chicago", now).unwrap();
        assert_eq!(start, ms_from_ymd(chi, 2026, 3, 1));
    }

    #[test]
    fn month_bounds_tokyo() {
        // 2026-01-31T20:00:00Z is 2026-02-01 05:00 in Tokyo — month must be Feb.
        let now = chrono::Utc
            .with_ymd_and_hms(2026, 1, 31, 20, 0, 0)
            .unwrap()
            .timestamp_millis();
        let tyo: Tz = "Asia/Tokyo".parse().unwrap();
        let (start, _end) = current_month_bounds_ms("Asia/Tokyo", now).unwrap();
        assert_eq!(start, ms_from_ymd(tyo, 2026, 2, 1));
    }

    #[test]
    fn month_bounds_february_leap_year() {
        let now = ms_from_ymd(chrono_tz::UTC, 2024, 2, 10);
        let (_start, end) = current_month_bounds_ms("UTC", now).unwrap();
        assert_eq!(end, ms_from_ymd(chrono_tz::UTC, 2024, 2, 29));
    }

    #[test]
    fn month_bounds_dst_spring_forward_chicago() {
        let chi: Tz = "America/Chicago".parse().unwrap();
        let now = chi
            .with_ymd_and_hms(2026, 3, 10, 12, 0, 0)
            .unwrap()
            .timestamp_millis();
        let res = current_month_bounds_ms("America/Chicago", now);
        assert!(res.is_ok(), "expected Ok, got {:?}", res);
    }

    #[test]
    fn month_bounds_invalid_tz_errors() {
        let res = current_month_bounds_ms("Not/A_Zone", 0);
        assert!(res.is_err());
    }
}
