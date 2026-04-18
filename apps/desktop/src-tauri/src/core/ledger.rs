// Double-entry ledger engine — T-014, T-018
// Validates and commits TransactionProposals. No Tauri deps.

use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::SqlitePool;
use thiserror::Error;

use crate::core::proposal::{ProposedLine, Side, TransactionProposal};
use crate::core::validation::{validate_proposal, ValidationResult};
use crate::id::new_ulid;

#[derive(Debug, Error)]
pub enum LedgerError {
    #[error("Proposal validation failed")]
    ValidationFailed(ValidationResult),
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("An opening balance already exists for this account")]
    OpeningBalanceExists,
}

/// Parameters for creating an immutable opening balance entry.
pub struct OpeningBalanceRequest {
    /// The account receiving its initial balance.
    pub account_id: String,
    /// Counterpart account (typically an Opening Balance Equity account).
    pub equity_account_id: String,
    pub amount_cents: i64,
    /// Side for `account_id`; `equity_account_id` receives the opposite side.
    pub primary_side: Side,
    pub txn_date_ms: i64,
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

/// Creates an immutable opening balance for `account_id`.
///
/// Fails with [`LedgerError::OpeningBalanceExists`] if any `opening_balance`
/// transaction already touches the same account — enforcing single-write
/// immutability at the DB layer rather than relying on callers.
pub async fn create_opening_balance(
    pool: &SqlitePool,
    household_id: &str,
    req: &OpeningBalanceRequest,
) -> Result<String, LedgerError> {
    let (existing,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transactions t
         JOIN journal_lines jl ON jl.transaction_id = t.id
         WHERE t.source = 'opening_balance' AND jl.account_id = ?",
    )
    .bind(&req.account_id)
    .fetch_one(pool)
    .await?;

    if existing > 0 {
        return Err(LedgerError::OpeningBalanceExists);
    }

    let opposite_side = match req.primary_side {
        Side::Debit => Side::Credit,
        Side::Credit => Side::Debit,
    };

    let proposal = TransactionProposal {
        memo: Some("Opening balance".to_string()),
        txn_date_ms: req.txn_date_ms,
        lines: vec![
            ProposedLine {
                account_id: req.account_id.clone(),
                envelope_id: None,
                amount_cents: req.amount_cents,
                side: req.primary_side,
            },
            ProposedLine {
                account_id: req.equity_account_id.clone(),
                envelope_id: None,
                amount_cents: req.amount_cents,
                side: opposite_side,
            },
        ],
    };

    let result = validate_proposal(pool, &proposal).await;
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
         VALUES (?, ?, ?, ?, 'posted', 'opening_balance', ?, ?)",
    )
    .bind(&txn_id)
    .bind(household_id)
    .bind(req.txn_date_ms)
    .bind(now)
    .bind("Opening balance")
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

    // -- T-018 helpers --

    async fn setup_opening_balance_accounts(pool: &SqlitePool) {
        sqlx::query(
            "INSERT INTO households (id, name, timezone, created_at) VALUES ('hh_ob', 'Test', 'UTC', 0)",
        )
        .execute(pool)
        .await
        .expect("household");

        for (id, normal_balance, acct_type) in [
            ("acc_bank", "debit", "asset"),
            ("acc_equity", "credit", "equity"),
        ] {
            sqlx::query(
                "INSERT INTO accounts (id, household_id, name, type, normal_balance, is_placeholder, created_at)
                 VALUES (?, 'hh_ob', ?, ?, ?, 0, 0)",
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

    fn ob_request() -> OpeningBalanceRequest {
        OpeningBalanceRequest {
            account_id: "acc_bank".to_string(),
            equity_account_id: "acc_equity".to_string(),
            amount_cents: 50000,
            primary_side: Side::Debit,
            txn_date_ms: 1_700_000_000_000,
        }
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

    // -- T-018: create_opening_balance tests --

    #[tokio::test]
    async fn opening_balance_returns_ulid() {
        let pool = test_pool().await;
        setup_opening_balance_accounts(&pool).await;

        let txn_id = create_opening_balance(&pool, "hh_ob", &ob_request())
            .await
            .expect("create ob");

        assert_eq!(txn_id.len(), 26);
        assert_eq!(txn_id, txn_id.to_uppercase());
    }

    #[tokio::test]
    async fn opening_balance_source_is_opening_balance() {
        let pool = test_pool().await;
        setup_opening_balance_accounts(&pool).await;

        let txn_id = create_opening_balance(&pool, "hh_ob", &ob_request())
            .await
            .expect("create ob");

        let (source,): (String,) =
            sqlx::query_as("SELECT source FROM transactions WHERE id = ?")
                .bind(&txn_id)
                .fetch_one(&pool)
                .await
                .expect("query");

        assert_eq!(source, "opening_balance");
    }

    #[tokio::test]
    async fn opening_balance_inserts_two_journal_lines() {
        let pool = test_pool().await;
        setup_opening_balance_accounts(&pool).await;

        let txn_id = create_opening_balance(&pool, "hh_ob", &ob_request())
            .await
            .expect("create ob");

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
    async fn opening_balance_rejects_duplicate() {
        let pool = test_pool().await;
        setup_opening_balance_accounts(&pool).await;

        create_opening_balance(&pool, "hh_ob", &ob_request())
            .await
            .expect("first ob");

        let err = create_opening_balance(&pool, "hh_ob", &ob_request())
            .await
            .expect_err("second ob should fail");

        assert!(matches!(err, LedgerError::OpeningBalanceExists));
    }

    #[tokio::test]
    async fn opening_balance_rejects_zero_amount() {
        let pool = test_pool().await;
        setup_opening_balance_accounts(&pool).await;

        let req = OpeningBalanceRequest {
            amount_cents: 0,
            ..ob_request()
        };

        let err = create_opening_balance(&pool, "hh_ob", &req)
            .await
            .expect_err("zero amount should fail");

        assert!(matches!(err, LedgerError::ValidationFailed(_)));
    }

    #[tokio::test]
    async fn opening_balance_rejects_unknown_account() {
        let pool = test_pool().await;
        setup_opening_balance_accounts(&pool).await;

        let req = OpeningBalanceRequest {
            account_id: "no_such_account".to_string(),
            ..ob_request()
        };

        let err = create_opening_balance(&pool, "hh_ob", &req)
            .await
            .expect_err("unknown account should fail");

        assert!(matches!(err, LedgerError::ValidationFailed(_)));
    }

    #[tokio::test]
    async fn opening_balance_rejects_placeholder_account() {
        let pool = test_pool().await;
        setup_opening_balance_accounts(&pool).await;

        sqlx::query(
            "INSERT INTO accounts (id, household_id, name, type, normal_balance, is_placeholder, created_at)
             VALUES ('acc_placeholder', 'hh_ob', 'placeholder', 'asset', 'debit', 1, 0)",
        )
        .execute(&pool)
        .await
        .expect("insert placeholder");

        let req = OpeningBalanceRequest {
            account_id: "acc_placeholder".to_string(),
            ..ob_request()
        };

        let err = create_opening_balance(&pool, "hh_ob", &req)
            .await
            .expect_err("placeholder account should fail");

        assert!(matches!(err, LedgerError::ValidationFailed(_)));
    }

    #[tokio::test]
    async fn opening_balance_has_posted_status() {
        let pool = test_pool().await;
        setup_opening_balance_accounts(&pool).await;

        let txn_id = create_opening_balance(&pool, "hh_ob", &ob_request())
            .await
            .expect("create ob");

        let (status,): (String,) =
            sqlx::query_as("SELECT status FROM transactions WHERE id = ?")
                .bind(&txn_id)
                .fetch_one(&pool)
                .await
                .expect("query");

        assert_eq!(status, "posted");
    }
}
