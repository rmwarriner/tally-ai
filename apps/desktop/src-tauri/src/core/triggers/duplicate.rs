// Duplicate detection trigger — T-052.
//
// Looks for a posted transaction in the same household whose memo + total
// debits match the proposal, on the same household-local calendar day. The
// dedup key (entity_id) bundles those three fields so two distinct proposals
// against different prior posts each get their own alert.
//
// Issue #114 tracks tightening the broader validation `PossibleDuplicate`
// rule; this trigger is the user-facing surface for the trigger flavor.

use sqlx::SqlitePool;

use crate::core::insight::{day_bucket_ms, InsightDraft, InsightError, InsightKind};
use crate::core::proposal::{Side, TransactionProposal};

/// Returns a draft if the proposal looks like a duplicate of an existing
/// posted transaction. `None` if there's no match, the memo is empty, or
/// the proposal has zero debits.
pub async fn check(
    pool: &SqlitePool,
    household_id: &str,
    tz: &str,
    proposal: &TransactionProposal,
) -> Result<Option<InsightDraft>, InsightError> {
    let memo = match proposal.memo.as_deref().map(str::trim) {
        Some(m) if !m.is_empty() => m,
        _ => return Ok(None),
    };

    let debit_total: i64 = proposal
        .lines
        .iter()
        .filter(|l| matches!(l.side, Side::Debit))
        .map(|l| l.amount_cents)
        .sum();
    if debit_total == 0 {
        return Ok(None);
    }

    let day_bucket = day_bucket_ms(tz, proposal.txn_date_ms)?;
    let day_end = day_bucket + 86_400_000 - 1;

    // Same household, same calendar day, same memo (case-insensitive),
    // same total debit amount.
    let row: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT t.id
        FROM transactions t
        JOIN journal_lines jl ON jl.transaction_id = t.id
        WHERE t.household_id = ?
          AND t.status = 'posted'
          AND t.txn_date BETWEEN ? AND ?
          AND LOWER(IFNULL(t.memo, '')) = LOWER(?)
          AND jl.side = 'debit'
        GROUP BY t.id
        HAVING SUM(jl.amount) = ?
        LIMIT 1
        "#,
    )
    .bind(household_id)
    .bind(day_bucket)
    .bind(day_end)
    .bind(memo)
    .bind(debit_total)
    .fetch_optional(pool)
    .await?;

    let Some((existing_id,)) = row else {
        return Ok(None);
    };

    // Entity id bundles memo + amount + day so two distinct possible-dupes
    // on the same day each surface (different memos or amounts → different
    // entity ids).
    let entity_id = format!("{memo}|{debit_total}|{day_bucket}");

    Ok(Some(InsightDraft {
        kind: InsightKind::PossibleDuplicate,
        entity_id: Some(entity_id),
        user_message: format!(
            "Looks like \"{}\" for {} may already be posted today.",
            memo,
            format_cents(debit_total),
        ),
        extra: Some(serde_json::json!({
            "existing_transaction_id": existing_id,
            "memo": memo,
            "amount_cents": debit_total,
        })),
    }))
}

fn format_cents(cents: i64) -> String {
    let dollars = cents / 100;
    let remainder = (cents % 100).abs();
    format!("${dollars}.{remainder:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::proposal::ProposedLine;
    use crate::db::create_encrypted_db;
    use crate::id::new_ulid;
    use chrono::TimeZone;
    use chrono_tz::America::Chicago;
    use tempfile::tempdir;

    async fn test_pool() -> (SqlitePool, String) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("dup.db");
        let pool = create_encrypted_db(&path, "pp", &[0u8; 16]).await.unwrap();
        let hh = new_ulid();
        sqlx::query("INSERT INTO households (id, name, timezone, created_at) VALUES (?, 'H', 'America/Chicago', 0)")
            .bind(&hh).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO accounts (id, household_id, name, type, normal_balance, created_at) VALUES ('acc_grc', ?, 'Groceries', 'expense', 'debit', 0), ('acc_chk', ?, 'Checking', 'asset', 'debit', 0)")
            .bind(&hh).bind(&hh).execute(&pool).await.unwrap();
        std::mem::forget(dir);
        (pool, hh)
    }

    async fn seed_posted(
        pool: &SqlitePool,
        hh: &str,
        memo: &str,
        amount: i64,
        txn_date_ms: i64,
    ) -> String {
        let id = new_ulid();
        sqlx::query("INSERT INTO transactions (id, household_id, txn_date, entry_date, status, source, memo, created_at) VALUES (?, ?, ?, 0, 'posted', 'manual', ?, 0)")
            .bind(&id).bind(hh).bind(txn_date_ms).bind(memo).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO journal_lines (id, transaction_id, account_id, amount, side, created_at) VALUES (?, ?, 'acc_grc', ?, 'debit', 0), (?, ?, 'acc_chk', ?, 'credit', 0)")
            .bind(new_ulid()).bind(&id).bind(amount)
            .bind(new_ulid()).bind(&id).bind(amount)
            .execute(pool).await.unwrap();
        id
    }

    fn proposal(memo: &str, amount: i64, day: i64) -> TransactionProposal {
        TransactionProposal {
            memo: Some(memo.to_string()),
            txn_date_ms: day,
            lines: vec![
                ProposedLine { account_id: "acc_grc".into(), envelope_id: None, amount_cents: amount, side: Side::Debit },
                ProposedLine { account_id: "acc_chk".into(), envelope_id: None, amount_cents: amount, side: Side::Credit },
            ],
        }
    }

    fn day_ms(year: i32, month: u32, day: u32, hour: u32) -> i64 {
        Chicago
            .with_ymd_and_hms(year, month, day, hour, 0, 0)
            .unwrap()
            .timestamp_millis()
    }

    #[tokio::test]
    async fn returns_some_for_exact_match_same_day() {
        let (pool, hh) = test_pool().await;
        seed_posted(&pool, &hh, "Whole Foods", 4500, day_ms(2024, 5, 18, 10)).await;
        let p = proposal("Whole Foods", 4500, day_ms(2024, 5, 18, 16));
        let result = check(&pool, &hh, "America/Chicago", &p).await.unwrap();
        assert!(result.is_some());
        let d = result.unwrap();
        assert_eq!(d.kind, InsightKind::PossibleDuplicate);
        assert!(d.user_message.contains("Whole Foods"));
        assert!(d.user_message.contains("$45.00"));
    }

    #[tokio::test]
    async fn matches_case_insensitively() {
        let (pool, hh) = test_pool().await;
        seed_posted(&pool, &hh, "Whole Foods", 4500, day_ms(2024, 5, 18, 10)).await;
        let p = proposal("WHOLE FOODS", 4500, day_ms(2024, 5, 18, 16));
        assert!(check(&pool, &hh, "America/Chicago", &p).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn different_amount_returns_none() {
        let (pool, hh) = test_pool().await;
        seed_posted(&pool, &hh, "Whole Foods", 4500, day_ms(2024, 5, 18, 10)).await;
        let p = proposal("Whole Foods", 4501, day_ms(2024, 5, 18, 16));
        assert!(check(&pool, &hh, "America/Chicago", &p).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn different_day_returns_none() {
        let (pool, hh) = test_pool().await;
        seed_posted(&pool, &hh, "Whole Foods", 4500, day_ms(2024, 5, 17, 10)).await;
        let p = proposal("Whole Foods", 4500, day_ms(2024, 5, 18, 10));
        assert!(check(&pool, &hh, "America/Chicago", &p).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn different_memo_returns_none() {
        let (pool, hh) = test_pool().await;
        seed_posted(&pool, &hh, "Whole Foods", 4500, day_ms(2024, 5, 18, 10)).await;
        let p = proposal("Trader Joe's", 4500, day_ms(2024, 5, 18, 16));
        assert!(check(&pool, &hh, "America/Chicago", &p).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn empty_memo_returns_none_even_if_amount_matches() {
        let (pool, hh) = test_pool().await;
        seed_posted(&pool, &hh, "", 4500, day_ms(2024, 5, 18, 10)).await;
        let p = proposal("", 4500, day_ms(2024, 5, 18, 16));
        assert!(check(&pool, &hh, "America/Chicago", &p).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn different_household_returns_none() {
        let (pool, hh) = test_pool().await;
        let other_hh = new_ulid();
        sqlx::query("INSERT INTO households (id, name, timezone, created_at) VALUES (?, 'B', 'America/Chicago', 0)")
            .bind(&other_hh).execute(&pool).await.unwrap();
        seed_posted(&pool, &other_hh, "Whole Foods", 4500, day_ms(2024, 5, 18, 10)).await;
        let p = proposal("Whole Foods", 4500, day_ms(2024, 5, 18, 16));
        assert!(check(&pool, &hh, "America/Chicago", &p).await.unwrap().is_none());
    }
}
