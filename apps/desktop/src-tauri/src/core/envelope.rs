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

use sqlx::SqlitePool;

/// Inserts a new envelope and its current-month envelope_periods row.
/// Returns the new envelope ULID. Resolves or creates an expense account
/// for the envelope name (same behavior as before). Month bounds come from
/// `current_month_bounds_ms` using the household's IANA `timezone`.
pub async fn create_envelope_with_current_period(
    pool: &SqlitePool,
    household_id: &str,
    name: &str,
    now_ms: i64,
) -> Result<String, String> {
    use crate::id::new_ulid;

    // Look up the household timezone.
    let tz: (String,) =
        sqlx::query_as("SELECT timezone FROM households WHERE id = ?")
            .bind(household_id)
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;

    let (period_start, period_end) = current_month_bounds_ms(&tz.0, now_ms)?;

    // Resolve the target expense account: pick the first non-placeholder
    // expense account, or create a generic one under the Expenses placeholder.
    let expense_account: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM accounts WHERE household_id = ? AND type = 'expense' AND is_placeholder = 0 LIMIT 1",
    )
    .bind(household_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let account_id = if let Some((id,)) = expense_account {
        id
    } else {
        let id = new_ulid();
        let parent: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM accounts WHERE household_id = ? AND type = 'expense' AND is_placeholder = 1 AND name = 'Expenses' LIMIT 1",
        )
        .bind(household_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;

        sqlx::query(
            "INSERT INTO accounts (id, household_id, parent_id, name, type, normal_balance, is_placeholder, currency, created_at)
             VALUES (?, ?, ?, ?, 'expense', 'debit', 0, 'USD', ?)",
        )
        .bind(&id)
        .bind(household_id)
        .bind(parent.map(|(pid,)| pid))
        .bind(name)
        .bind(now_ms)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

        id
    };

    let envelope_id = new_ulid();
    sqlx::query(
        "INSERT INTO envelopes (id, household_id, account_id, name, created_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&envelope_id)
    .bind(household_id)
    .bind(&account_id)
    .bind(name)
    .bind(now_ms)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    sqlx::query(
        "INSERT INTO envelope_periods
           (id, envelope_id, period_start, period_end, allocated, spent, created_at)
         VALUES (?, ?, ?, ?, 0, 0, ?)",
    )
    .bind(new_ulid())
    .bind(&envelope_id)
    .bind(period_start)
    .bind(period_end)
    .bind(now_ms)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(envelope_id)
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

    #[tokio::test]
    async fn create_envelope_with_current_period_inserts_period_row() {
        use crate::db::connection::create_encrypted_db;
        use crate::db::migrations::run_migrations;
        use crate::id::new_ulid;
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let path = dir.path().join("env_test.db");
        let pool = create_encrypted_db(&path, "pw", &[0u8; 16]).await.unwrap();
        run_migrations(&pool).await.unwrap();

        let hid = new_ulid();
        sqlx::query(
            "INSERT INTO households (id, name, timezone, created_at) VALUES (?, 'Test', 'UTC', 0)",
        )
        .bind(&hid)
        .execute(&pool)
        .await
        .unwrap();

        let now = chrono::Utc
            .with_ymd_and_hms(2026, 4, 15, 12, 0, 0)
            .unwrap()
            .timestamp_millis();

        let env_id = create_envelope_with_current_period(&pool, &hid, "Groceries", now)
            .await
            .unwrap();

        let (start, end): (i64, i64) = sqlx::query_as(
            "SELECT period_start, period_end FROM envelope_periods WHERE envelope_id = ?",
        )
        .bind(&env_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        let utc: chrono_tz::Tz = "UTC".parse().unwrap();
        let expect_start = utc.with_ymd_and_hms(2026, 4, 1, 0, 0, 0).unwrap().timestamp_millis();
        let expect_end = utc.with_ymd_and_hms(2026, 4, 30, 0, 0, 0).unwrap().timestamp_millis();
        assert_eq!(start, expect_start);
        assert_eq!(end, expect_end);
    }
}
