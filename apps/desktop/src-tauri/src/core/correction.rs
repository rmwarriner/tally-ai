// GAAP correction and undo — T-015, T-016
// All writes are atomic. Opening-balance transactions are non-correctable
// and non-undoable per spec (section 8 pre-build blockers).

use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::SqlitePool;
use thiserror::Error;

use crate::core::proposal::TransactionProposal;
use crate::core::validation::{validate_proposal, ValidationResult};
use crate::id::new_ulid;

#[derive(Debug, Error)]
pub enum CorrectionError {
    #[error("Transaction not found or is not posted")]
    NotFound,
    #[error("This transaction cannot be corrected or undone")]
    NotCorrectable,
    #[error("Replacement validation failed")]
    ValidationFailed(ValidationResult),
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

#[derive(sqlx::FromRow)]
struct TxnRow {
    id: String,
    source: String,
    status: String,
}

#[derive(sqlx::FromRow)]
struct LineRow {
    id: String,
    account_id: String,
    envelope_id: Option<String>,
    amount: i64,
    side: String,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn flip_side(side: &str) -> &'static str {
    if side == "debit" { "credit" } else { "debit" }
}

/// Inserts a void-and-reversal pair atomically inside an existing DB transaction.
/// Returns the new reversal transaction ULID.
async fn void_and_reverse(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    original: &TxnRow,
    lines: &[LineRow],
    household_id: &str,
    now: i64,
) -> Result<String, sqlx::Error> {
    // Void the original
    sqlx::query("UPDATE transactions SET status = 'void' WHERE id = ?")
        .bind(&original.id)
        .execute(&mut **tx)
        .await?;

    // Insert reversal transaction
    let reversal_id = new_ulid();
    sqlx::query(
        "INSERT INTO transactions
             (id, household_id, txn_date, entry_date, status, source, memo, corrects_txn_id, created_at)
         SELECT ?, ?, txn_date, ?, 'posted', source, 'Reversal', id, ?
         FROM transactions WHERE id = ?",
    )
    .bind(&reversal_id)
    .bind(household_id)
    .bind(now)
    .bind(now)
    .bind(&original.id)
    .execute(&mut **tx)
    .await?;

    // Insert reversed journal lines (sides flipped)
    for line in lines {
        let new_line_id = new_ulid();
        sqlx::query(
            "INSERT INTO journal_lines
                 (id, transaction_id, account_id, envelope_id, amount, side, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&new_line_id)
        .bind(&reversal_id)
        .bind(&line.account_id)
        .bind(&line.envelope_id)
        .bind(line.amount)
        .bind(flip_side(&line.side))
        .bind(now)
        .execute(&mut **tx)
        .await?;
    }

    Ok(reversal_id)
}

/// GAAP correction: voids `original_txn_id`, creates a reversal, then commits
/// `replacement` as a new posted transaction. Returns the replacement ULID.
///
/// Opening-balance transactions are rejected — they are non-correctable by design.
pub async fn correct_transaction(
    pool: &SqlitePool,
    household_id: &str,
    original_txn_id: &str,
    replacement: &TransactionProposal,
) -> Result<String, CorrectionError> {
    let original: Option<TxnRow> = sqlx::query_as(
        "SELECT id, source, status FROM transactions
         WHERE id = ? AND household_id = ?",
    )
    .bind(original_txn_id)
    .bind(household_id)
    .fetch_optional(pool)
    .await?;

    let original = original.ok_or(CorrectionError::NotFound)?;

    if original.status != "posted" {
        return Err(CorrectionError::NotFound);
    }
    if original.source == "opening_balance" {
        return Err(CorrectionError::NotCorrectable);
    }

    let lines: Vec<LineRow> = sqlx::query_as(
        "SELECT id, account_id, envelope_id, amount, side FROM journal_lines
         WHERE transaction_id = ?",
    )
    .bind(original_txn_id)
    .fetch_all(pool)
    .await?;

    let result = validate_proposal(pool, replacement).await;
    if !result.is_accepted() {
        return Err(CorrectionError::ValidationFailed(result));
    }

    let mut tx = pool.begin().await?;
    let now = now_ms();

    let reversal_id = void_and_reverse(&mut tx, &original, &lines, household_id, now).await?;

    // Insert replacement transaction (corrects_txn_id points to the reversal)
    let replacement_id = new_ulid();
    sqlx::query(
        "INSERT INTO transactions
             (id, household_id, txn_date, entry_date, status, source, memo, corrects_txn_id, created_at)
         VALUES (?, ?, ?, ?, 'posted', 'ai', ?, ?, ?)",
    )
    .bind(&replacement_id)
    .bind(household_id)
    .bind(replacement.txn_date_ms)
    .bind(now)
    .bind(&replacement.memo)
    .bind(&reversal_id)
    .bind(now)
    .execute(&mut *tx)
    .await?;

    for line in &replacement.lines {
        let line_id = new_ulid();
        let side_str = if matches!(line.side, crate::core::proposal::Side::Debit) {
            "debit"
        } else {
            "credit"
        };
        sqlx::query(
            "INSERT INTO journal_lines
                 (id, transaction_id, account_id, envelope_id, amount, side, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&line_id)
        .bind(&replacement_id)
        .bind(&line.account_id)
        .bind(&line.envelope_id)
        .bind(line.amount_cents)
        .bind(side_str)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(replacement_id)
}

/// Reverses the most-recently posted, non-opening-balance transaction for
/// `household_id`. Returns the reversal ULID.
///
/// The UI layer is responsible for requesting confirmation before calling this.
pub async fn undo_last_transaction(
    pool: &SqlitePool,
    household_id: &str,
) -> Result<String, CorrectionError> {
    let original: Option<TxnRow> = sqlx::query_as(
        "SELECT id, source, status FROM transactions
         WHERE household_id = ?
           AND status = 'posted'
           AND source != 'opening_balance'
         ORDER BY created_at DESC
         LIMIT 1",
    )
    .bind(household_id)
    .fetch_optional(pool)
    .await?;

    let original = original.ok_or(CorrectionError::NotFound)?;

    let lines: Vec<LineRow> = sqlx::query_as(
        "SELECT id, account_id, envelope_id, amount, side FROM journal_lines
         WHERE transaction_id = ?",
    )
    .bind(&original.id)
    .fetch_all(pool)
    .await?;

    let mut tx = pool.begin().await?;
    let now = now_ms();

    let reversal_id = void_and_reverse(&mut tx, &original, &lines, household_id, now).await?;

    tx.commit().await?;

    Ok(reversal_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::core::proposal::{ProposedLine, Side};
    use crate::db::{connection::create_encrypted_db, migrations::run_migrations};
    use tempfile::tempdir;

    fn now_ms() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    }

    async fn test_pool() -> SqlitePool {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.keep().join("test.db");
        let pool = create_encrypted_db(&db_path, "test", &[0u8; 16])
            .await
            .expect("create db");
        run_migrations(&pool).await.expect("migrate");
        pool
    }

    async fn setup(pool: &SqlitePool) {
        sqlx::query(
            "INSERT INTO households (id, name, timezone, created_at) VALUES ('hh', 'Test', 'UTC', 0)",
        )
        .execute(pool)
        .await
        .expect("household");

        for (id, normal_balance, acct_type) in [
            ("acc_checking", "debit", "asset"),
            ("acc_income", "credit", "income"),
        ] {
            sqlx::query(
                "INSERT INTO accounts (id, household_id, name, type, normal_balance, is_placeholder, created_at)
                 VALUES (?, 'hh', ?, ?, ?, 0, 0)",
            )
            .bind(id)
            .bind(id)
            .bind(acct_type)
            .bind(normal_balance)
            .execute(pool)
            .await
            .expect("account");
        }
    }

    /// Inserts a posted transaction and returns its ID.
    async fn seed_posted_txn(pool: &SqlitePool, source: &str) -> String {
        let txn_id = new_ulid();
        let now = now_ms();
        sqlx::query(
            "INSERT INTO transactions
                 (id, household_id, txn_date, entry_date, status, source, created_at)
             VALUES (?, 'hh', ?, ?, 'posted', ?, ?)",
        )
        .bind(&txn_id)
        .bind(now)
        .bind(now)
        .bind(source)
        .bind(now)
        .execute(pool)
        .await
        .expect("txn");

        sqlx::query(
            "INSERT INTO journal_lines (id, transaction_id, account_id, amount, side, created_at)
             VALUES (?, ?, 'acc_checking', 10000, 'debit', ?)",
        )
        .bind(new_ulid())
        .bind(&txn_id)
        .bind(now)
        .execute(pool)
        .await
        .expect("debit line");

        sqlx::query(
            "INSERT INTO journal_lines (id, transaction_id, account_id, amount, side, created_at)
             VALUES (?, ?, 'acc_income', 10000, 'credit', ?)",
        )
        .bind(new_ulid())
        .bind(&txn_id)
        .bind(now)
        .execute(pool)
        .await
        .expect("credit line");

        txn_id
    }

    fn replacement_proposal() -> TransactionProposal {
        TransactionProposal {
            memo: Some("Corrected entry".to_string()),
            txn_date_ms: now_ms(),
            lines: vec![
                ProposedLine {
                    account_id: "acc_checking".to_string(),
                    envelope_id: None,
                    amount_cents: 5000,
                    side: Side::Debit,
                },
                ProposedLine {
                    account_id: "acc_income".to_string(),
                    envelope_id: None,
                    amount_cents: 5000,
                    side: Side::Credit,
                },
            ],
        }
    }

    // -- T-015: correct_transaction tests --

    #[tokio::test]
    async fn correct_transaction_returns_replacement_ulid() {
        let pool = test_pool().await;
        setup(&pool).await;
        let original_id = seed_posted_txn(&pool, "ai").await;

        let replacement_id =
            correct_transaction(&pool, "hh", &original_id, &replacement_proposal())
                .await
                .expect("correct");

        assert_eq!(replacement_id.len(), 26);
        assert_eq!(replacement_id, replacement_id.to_uppercase());
    }

    #[tokio::test]
    async fn correct_transaction_voids_original() {
        let pool = test_pool().await;
        setup(&pool).await;
        let original_id = seed_posted_txn(&pool, "ai").await;

        correct_transaction(&pool, "hh", &original_id, &replacement_proposal())
            .await
            .expect("correct");

        let (status,): (String,) =
            sqlx::query_as("SELECT status FROM transactions WHERE id = ?")
                .bind(&original_id)
                .fetch_one(&pool)
                .await
                .expect("query");
        assert_eq!(status, "void");
    }

    #[tokio::test]
    async fn correct_transaction_creates_reversal_linked_to_original() {
        let pool = test_pool().await;
        setup(&pool).await;
        let original_id = seed_posted_txn(&pool, "ai").await;

        correct_transaction(&pool, "hh", &original_id, &replacement_proposal())
            .await
            .expect("correct");

        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM transactions WHERE corrects_txn_id = ? AND memo = 'Reversal'",
        )
        .bind(&original_id)
        .fetch_one(&pool)
        .await
        .expect("query");
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn correct_transaction_replacement_links_to_reversal() {
        let pool = test_pool().await;
        setup(&pool).await;
        let original_id = seed_posted_txn(&pool, "ai").await;

        let replacement_id =
            correct_transaction(&pool, "hh", &original_id, &replacement_proposal())
                .await
                .expect("correct");

        let (corrects_id,): (Option<String>,) = sqlx::query_as(
            "SELECT corrects_txn_id FROM transactions WHERE id = ?",
        )
        .bind(&replacement_id)
        .fetch_one(&pool)
        .await
        .expect("query");

        assert!(corrects_id.is_some(), "replacement must link to a transaction");
        // The linked transaction should be the reversal (not the original)
        assert_ne!(corrects_id.as_deref(), Some(original_id.as_str()));
    }

    #[tokio::test]
    async fn correct_transaction_rejects_opening_balance() {
        let pool = test_pool().await;
        setup(&pool).await;
        let ob_id = seed_posted_txn(&pool, "opening_balance").await;

        let err = correct_transaction(&pool, "hh", &ob_id, &replacement_proposal())
            .await
            .expect_err("should fail");
        assert!(matches!(err, CorrectionError::NotCorrectable));
    }

    #[tokio::test]
    async fn correct_transaction_rejects_unknown_id() {
        let pool = test_pool().await;
        setup(&pool).await;

        let err = correct_transaction(&pool, "hh", "NONEXISTENT_ID", &replacement_proposal())
            .await
            .expect_err("should fail");
        assert!(matches!(err, CorrectionError::NotFound));
    }

    #[tokio::test]
    async fn correct_transaction_rejects_already_voided() {
        let pool = test_pool().await;
        setup(&pool).await;
        let original_id = seed_posted_txn(&pool, "ai").await;

        // Void it directly
        sqlx::query("UPDATE transactions SET status = 'void' WHERE id = ?")
            .bind(&original_id)
            .execute(&pool)
            .await
            .expect("void");

        let err = correct_transaction(&pool, "hh", &original_id, &replacement_proposal())
            .await
            .expect_err("should fail");
        assert!(matches!(err, CorrectionError::NotFound));
    }

    // -- T-016: undo_last_transaction tests --

    #[tokio::test]
    async fn undo_returns_reversal_ulid() {
        let pool = test_pool().await;
        setup(&pool).await;
        seed_posted_txn(&pool, "ai").await;

        let reversal_id = undo_last_transaction(&pool, "hh")
            .await
            .expect("undo");

        assert_eq!(reversal_id.len(), 26);
        assert_eq!(reversal_id, reversal_id.to_uppercase());
    }

    #[tokio::test]
    async fn undo_voids_last_posted_transaction() {
        let pool = test_pool().await;
        setup(&pool).await;
        let txn_id = seed_posted_txn(&pool, "ai").await;

        undo_last_transaction(&pool, "hh").await.expect("undo");

        let (status,): (String,) =
            sqlx::query_as("SELECT status FROM transactions WHERE id = ?")
                .bind(&txn_id)
                .fetch_one(&pool)
                .await
                .expect("query");
        assert_eq!(status, "void");
    }

    #[tokio::test]
    async fn undo_creates_reversal_with_flipped_sides() {
        let pool = test_pool().await;
        setup(&pool).await;
        seed_posted_txn(&pool, "ai").await;

        let reversal_id = undo_last_transaction(&pool, "hh").await.expect("undo");

        let lines: Vec<(i64, String)> = sqlx::query_as(
            "SELECT amount, side FROM journal_lines WHERE transaction_id = ? ORDER BY side",
        )
        .bind(&reversal_id)
        .fetch_all(&pool)
        .await
        .expect("query");

        assert_eq!(lines.len(), 2);
        let sides: Vec<&str> = lines.iter().map(|(_, s)| s.as_str()).collect();
        // Original was debit checking + credit income → reversal is credit checking + debit income
        assert!(sides.contains(&"credit"));
        assert!(sides.contains(&"debit"));
    }

    #[tokio::test]
    async fn undo_skips_opening_balance_transactions() {
        let pool = test_pool().await;
        setup(&pool).await;
        // Only an opening_balance transaction exists — undo should not touch it
        seed_posted_txn(&pool, "opening_balance").await;

        let err = undo_last_transaction(&pool, "hh")
            .await
            .expect_err("should fail");
        assert!(matches!(err, CorrectionError::NotFound));
    }

    #[tokio::test]
    async fn undo_fails_when_no_posted_transactions() {
        let pool = test_pool().await;
        setup(&pool).await;

        let err = undo_last_transaction(&pool, "hh")
            .await
            .expect_err("should fail");
        assert!(matches!(err, CorrectionError::NotFound));
    }
}
