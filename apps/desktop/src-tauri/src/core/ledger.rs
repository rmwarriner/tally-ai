// Double-entry ledger engine — T-014
// Validates and commits TransactionProposals. No Tauri deps.

use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::SqlitePool;
use thiserror::Error;

use crate::core::proposal::{Side, TransactionProposal};
use crate::core::validation::{validate_proposal, ValidationResult};
use crate::id::new_ulid;

#[derive(Debug, Error)]
pub enum LedgerError {
    #[error("Proposal validation failed")]
    ValidationFailed(ValidationResult),
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Validates `proposal` then atomically writes a `transactions` row and all
/// `journal_lines` rows. Returns the new transaction ULID on success.
pub async fn commit_proposal(
    pool: &SqlitePool,
    household_id: &str,
    proposal: &TransactionProposal,
) -> Result<String, LedgerError> {
    let result = validate_proposal(pool, proposal).await;
    if !result.is_accepted() {
        return Err(LedgerError::ValidationFailed(result));
    }

    let mut tx = pool.begin().await?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let txn_id = new_ulid();

    sqlx::query(
        "INSERT INTO transactions
             (id, household_id, txn_date, entry_date, status, source, memo, created_at)
         VALUES (?, ?, ?, ?, 'posted', 'ai', ?, ?)",
    )
    .bind(&txn_id)
    .bind(household_id)
    .bind(proposal.txn_date_ms)
    .bind(now)
    .bind(&proposal.memo)
    .bind(now)
    .execute(&mut *tx)
    .await?;

    for line in &proposal.lines {
        let line_id = new_ulid();
        let side_str = match line.side {
            Side::Debit => "debit",
            Side::Credit => "credit",
        };
        sqlx::query(
            "INSERT INTO journal_lines
                 (id, transaction_id, account_id, envelope_id, amount, side, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&line_id)
        .bind(&txn_id)
        .bind(&line.account_id)
        .bind(&line.envelope_id)
        .bind(line.amount_cents)
        .bind(side_str)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(txn_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::proposal::ProposedLine;
    use crate::db::{connection::create_encrypted_db, migrations::run_migrations};
    use tempfile::tempdir;

    async fn test_pool() -> SqlitePool {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.keep().join("test.db");
        let pool = create_encrypted_db(&db_path, "test", &[0u8; 16])
            .await
            .expect("create db");
        run_migrations(&pool).await.expect("migrate");
        pool
    }

    async fn setup_household_and_accounts(pool: &SqlitePool) {
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

    fn valid_proposal() -> TransactionProposal {
        TransactionProposal {
            memo: Some("Test income".to_string()),
            txn_date_ms: 1_700_000_000_000,
            lines: vec![
                ProposedLine {
                    account_id: "acc_checking".to_string(),
                    envelope_id: None,
                    amount_cents: 10000,
                    side: Side::Debit,
                },
                ProposedLine {
                    account_id: "acc_income".to_string(),
                    envelope_id: None,
                    amount_cents: 10000,
                    side: Side::Credit,
                },
            ],
        }
    }

    #[tokio::test]
    async fn commit_returns_transaction_ulid() {
        let pool = test_pool().await;
        setup_household_and_accounts(&pool).await;

        let txn_id = commit_proposal(&pool, "hh", &valid_proposal())
            .await
            .expect("commit");

        assert_eq!(txn_id.len(), 26, "ULID should be 26 chars");
        assert_eq!(txn_id, txn_id.to_uppercase(), "ULID should be uppercase");
    }

    #[tokio::test]
    async fn commit_inserts_transaction_row() {
        let pool = test_pool().await;
        setup_household_and_accounts(&pool).await;

        let txn_id = commit_proposal(&pool, "hh", &valid_proposal())
            .await
            .expect("commit");

        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE id = ?")
                .bind(&txn_id)
                .fetch_one(&pool)
                .await
                .expect("query");

        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn commit_inserts_journal_lines() {
        let pool = test_pool().await;
        setup_household_and_accounts(&pool).await;

        let txn_id = commit_proposal(&pool, "hh", &valid_proposal())
            .await
            .expect("commit");

        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM journal_lines WHERE transaction_id = ?",
        )
        .bind(&txn_id)
        .fetch_one(&pool)
        .await
        .expect("query");

        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn committed_transaction_has_posted_status() {
        let pool = test_pool().await;
        setup_household_and_accounts(&pool).await;

        let txn_id = commit_proposal(&pool, "hh", &valid_proposal())
            .await
            .expect("commit");

        let (status,): (String,) =
            sqlx::query_as("SELECT status FROM transactions WHERE id = ?")
                .bind(&txn_id)
                .fetch_one(&pool)
                .await
                .expect("query");

        assert_eq!(status, "posted");
    }

    #[tokio::test]
    async fn committed_transaction_has_ai_source() {
        let pool = test_pool().await;
        setup_household_and_accounts(&pool).await;

        let txn_id = commit_proposal(&pool, "hh", &valid_proposal())
            .await
            .expect("commit");

        let (source,): (String,) =
            sqlx::query_as("SELECT source FROM transactions WHERE id = ?")
                .bind(&txn_id)
                .fetch_one(&pool)
                .await
                .expect("query");

        assert_eq!(source, "ai");
    }

    #[tokio::test]
    async fn commit_lines_have_correct_amounts_and_sides() {
        let pool = test_pool().await;
        setup_household_and_accounts(&pool).await;

        let txn_id = commit_proposal(&pool, "hh", &valid_proposal())
            .await
            .expect("commit");

        let lines: Vec<(i64, String)> = sqlx::query_as(
            "SELECT amount, side FROM journal_lines WHERE transaction_id = ? ORDER BY side",
        )
        .bind(&txn_id)
        .fetch_all(&pool)
        .await
        .expect("query");

        assert_eq!(lines.len(), 2);
        // Both amounts should be 10000
        assert!(lines.iter().all(|(amt, _)| *amt == 10000));
        // One debit, one credit
        let sides: Vec<&str> = lines.iter().map(|(_, s)| s.as_str()).collect();
        assert!(sides.contains(&"debit"));
        assert!(sides.contains(&"credit"));
    }

    #[tokio::test]
    async fn commit_rejects_invalid_proposal_with_no_db_write() {
        let pool = test_pool().await;
        setup_household_and_accounts(&pool).await;

        // Invalid: unbalanced (100 debit vs 200 credit)
        let bad_proposal = TransactionProposal {
            memo: None,
            txn_date_ms: 1_700_000_000_000,
            lines: vec![
                ProposedLine {
                    account_id: "acc_checking".to_string(),
                    envelope_id: None,
                    amount_cents: 10000,
                    side: Side::Debit,
                },
                ProposedLine {
                    account_id: "acc_income".to_string(),
                    envelope_id: None,
                    amount_cents: 20000,
                    side: Side::Credit,
                },
            ],
        };

        let err = commit_proposal(&pool, "hh", &bad_proposal)
            .await
            .expect_err("should fail");

        assert!(matches!(err, LedgerError::ValidationFailed(_)));

        let (txn_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM transactions")
                .fetch_one(&pool)
                .await
                .expect("query");
        assert_eq!(txn_count, 0, "no transaction should be written on rejection");
    }

    #[tokio::test]
    async fn commit_memo_is_stored() {
        let pool = test_pool().await;
        setup_household_and_accounts(&pool).await;

        let txn_id = commit_proposal(&pool, "hh", &valid_proposal())
            .await
            .expect("commit");

        let (memo,): (Option<String>,) =
            sqlx::query_as("SELECT memo FROM transactions WHERE id = ?")
                .bind(&txn_id)
                .fetch_one(&pool)
                .await
                .expect("query");

        assert_eq!(memo.as_deref(), Some("Test income"));
    }
}
