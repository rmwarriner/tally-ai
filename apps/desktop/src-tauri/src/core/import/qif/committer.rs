//! Atomically commits a [`QifImportPlan`] to the Tally database.
//!
//! Same atomicity contract as the GnuCash committer: a single SQL transaction
//! wraps every insert. Failures roll back accounts, transactions, and journal
//! lines together. The QIF committer additionally writes one `audit_log` row
//! per imported transaction (closing the gap flagged in CLAUDE.md for the
//! GnuCash side).

use serde_json::json;
use sqlx::{Acquire, SqlitePool};

use super::{
    AccountType, NormalBalance, QifError, QifImportPlan, QifImportReceipt, Side,
};
use crate::core::audit::{self, AuditAction};
use crate::id::new_ulid;

pub async fn commit(
    pool: &SqlitePool,
    plan: &QifImportPlan,
    skipped_security_trades: u32,
    now_ms: i64,
) -> Result<QifImportReceipt, QifError> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;

    // 1. Insert all mapped accounts (real + synthesized). QIF has no
    //    hierarchy and no upstream GUID, so parent_id is NULL and gnc_guid
    //    is NULL too.
    for m in &plan.account_mappings {
        sqlx::query(
            "INSERT INTO accounts (id, household_id, parent_id, name, type, normal_balance, is_placeholder, currency, created_at, import_id)
             VALUES (?, ?, NULL, ?, ?, ?, 0, 'USD', ?, ?)",
        )
        .bind(&m.tally_account_id)
        .bind(&plan.household_id)
        .bind(&m.tally_name)
        .bind(account_type_str(m.tally_type))
        .bind(normal_balance_str(m.tally_normal_balance))
        .bind(now_ms)
        .bind(&plan.import_id)
        .execute(&mut *tx)
        .await?;
    }

    let mut committed: u32 = 0;
    let mut skipped: u32 = 0;

    for ptx in &plan.transactions {
        // Idempotency check on (household_id, source_ref).
        let exists: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM transactions WHERE household_id = ? AND source_ref = ?",
        )
        .bind(&plan.household_id)
        .bind(&ptx.source_ref)
        .fetch_one(&mut *tx)
        .await?;
        if exists.0 > 0 {
            skipped += 1;
            continue;
        }

        let txn_ulid = new_ulid();
        sqlx::query(
            "INSERT INTO transactions (id, household_id, txn_date, entry_date, status, source, memo, import_id, source_ref, created_at)
             VALUES (?, ?, ?, ?, 'posted', 'import', ?, ?, ?, ?)",
        )
        .bind(&txn_ulid)
        .bind(&plan.household_id)
        .bind(ptx.txn_date)
        .bind(now_ms)
        .bind(&ptx.memo)
        .bind(&plan.import_id)
        .bind(&ptx.source_ref)
        .bind(now_ms)
        .execute(&mut *tx)
        .await?;

        for line in &ptx.lines {
            sqlx::query(
                "INSERT INTO journal_lines (id, transaction_id, account_id, envelope_id, amount, side, created_at)
                 VALUES (?, ?, ?, NULL, ?, ?, ?)",
            )
            .bind(new_ulid())
            .bind(&txn_ulid)
            .bind(&line.tally_account_id)
            .bind(line.amount_cents)
            .bind(side_str(line.side))
            .bind(now_ms)
            .execute(&mut *tx)
            .await?;
        }

        // audit_log row in the same transaction. Failure aborts the whole
        // commit per #143 — no silent compliance holes.
        audit::write(
            &mut tx,
            &plan.household_id,
            "transactions",
            &txn_ulid,
            AuditAction::Insert,
            &json!({
                "source": "qif_import",
                "import_id": plan.import_id,
                "source_ref": ptx.source_ref,
                "txn_date": ptx.txn_date,
                "memo": ptx.memo,
                "lines": ptx.lines.iter().map(|l| json!({
                    "account_id": l.tally_account_id,
                    "amount_cents": l.amount_cents,
                    "side": side_str(l.side),
                })).collect::<Vec<_>>(),
            }),
            now_ms,
        )
        .await?;

        committed += 1;
    }

    tx.commit().await?;

    Ok(QifImportReceipt {
        import_id: plan.import_id.clone(),
        accounts_created: plan.account_mappings.len() as u32,
        transactions_committed: committed,
        transactions_skipped: skipped,
        skipped_security_trades,
    })
}

pub async fn rollback(pool: &SqlitePool, import_id: &str) -> Result<(), QifError> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;

    sqlx::query(
        "DELETE FROM journal_lines WHERE transaction_id IN (SELECT id FROM transactions WHERE import_id = ?)",
    )
    .bind(import_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM transactions WHERE import_id = ?")
        .bind(import_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query("DELETE FROM accounts WHERE import_id = ?")
        .bind(import_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

fn account_type_str(t: AccountType) -> &'static str {
    match t {
        AccountType::Asset => "asset",
        AccountType::Liability => "liability",
        AccountType::Income => "income",
        AccountType::Expense => "expense",
        AccountType::Equity => "equity",
    }
}

fn normal_balance_str(n: NormalBalance) -> &'static str {
    match n {
        NormalBalance::Debit => "debit",
        NormalBalance::Credit => "credit",
    }
}

fn side_str(s: Side) -> &'static str {
    match s {
        Side::Debit => "debit",
        Side::Credit => "credit",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::import::qif::mapper::build_default_plan;
    use crate::core::import::qif::reader;
    use crate::core::import::qif::test_fixtures;
    use crate::db::{connection::create_encrypted_db, migrations::run_migrations};
    use tempfile::tempdir;

    async fn setup_db() -> (tempfile::TempDir, sqlx::SqlitePool, String) {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("tally.db");
        let pool = create_encrypted_db(&db_path, "pp", &[0u8; 16]).await.unwrap();
        run_migrations(&pool).await.unwrap();
        let hh_id = new_ulid();
        sqlx::query(
            "INSERT INTO households (id, name, timezone, created_at) VALUES (?, 'H', 'America/Chicago', 0)",
        )
        .bind(&hh_id)
        .execute(&pool)
        .await
        .unwrap();
        (dir, pool, hh_id)
    }

    #[tokio::test]
    async fn happy_path_commits_accounts_and_transactions() {
        let (_dir, pool, hh_id) = setup_db().await;
        let book = reader::parse(test_fixtures::banktivity_minimal()).unwrap();
        let plan = build_default_plan(hh_id.clone(), new_ulid(), &book, new_ulid).unwrap();
        let real_accounts = book.accounts.len();
        let synth_accounts = plan.account_mappings.len() - real_accounts;
        assert!(synth_accounts > 0, "expected at least the equity opening balance synth");

        let receipt = commit(&pool, &plan, 0, 100).await.unwrap();
        assert_eq!(receipt.accounts_created as usize, plan.account_mappings.len());
        assert_eq!(receipt.transactions_committed as usize, plan.transactions.len());
        assert_eq!(receipt.transactions_skipped, 0);

        let (acc_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE household_id = ?")
                .bind(&hh_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(acc_count as usize, plan.account_mappings.len());

        let (txn_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM transactions WHERE household_id = ? AND source = 'import'",
        )
        .bind(&hh_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(txn_count as usize, plan.transactions.len());
    }

    #[tokio::test]
    async fn idempotent_second_run_skips_all_transactions() {
        let (_dir, pool, hh_id) = setup_db().await;
        let book = reader::parse(test_fixtures::banktivity_minimal()).unwrap();
        let plan1 = build_default_plan(hh_id.clone(), new_ulid(), &book, new_ulid).unwrap();
        commit(&pool, &plan1, 0, 100).await.unwrap();

        // Second commit with fresh import_id and fresh account ULIDs.
        let plan2 = build_default_plan(hh_id.clone(), new_ulid(), &book, new_ulid).unwrap();
        let second = commit(&pool, &plan2, 0, 200).await.unwrap();
        assert_eq!(second.transactions_committed, 0);
        assert_eq!(second.transactions_skipped as usize, plan1.transactions.len());
    }

    #[tokio::test]
    async fn commit_writes_audit_log_row_per_transaction() {
        let (_dir, pool, hh_id) = setup_db().await;
        let book = reader::parse(test_fixtures::banktivity_minimal()).unwrap();
        let plan = build_default_plan(hh_id.clone(), new_ulid(), &book, new_ulid).unwrap();
        commit(&pool, &plan, 0, 100).await.unwrap();

        let (audit_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM audit_log WHERE household_id = ? AND table_name = 'transactions' AND action = 'insert'",
        )
        .bind(&hh_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(audit_count as usize, plan.transactions.len());

        let (sample_payload,): (String,) = sqlx::query_as(
            "SELECT payload FROM audit_log WHERE household_id = ? AND table_name = 'transactions' LIMIT 1",
        )
        .bind(&hh_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(sample_payload.contains("qif_import"));
        assert!(sample_payload.contains("source_ref"));
    }

    #[tokio::test]
    async fn commit_rolls_back_on_failure() {
        let (_dir, pool, hh_id) = setup_db().await;
        let book = reader::parse(test_fixtures::banktivity_minimal()).unwrap();
        let mut plan = build_default_plan(hh_id.clone(), new_ulid(), &book, new_ulid).unwrap();

        // Corrupt the plan: point the last txn's first line at a nonexistent account.
        let n = plan.transactions.len() - 1;
        plan.transactions[n].lines[0].tally_account_id = "DOES_NOT_EXIST".into();

        let err = commit(&pool, &plan, 0, 100).await.unwrap_err();
        assert!(matches!(err, QifError::Database(_)));

        let (acc_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE household_id = ?")
                .bind(&hh_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(acc_count, 0, "rollback must remove all inserted accounts");

        let (txn_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE household_id = ?")
                .bind(&hh_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(txn_count, 0, "rollback must remove all inserted transactions");

        let (audit_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM audit_log WHERE household_id = ?")
                .bind(&hh_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(audit_count, 0, "rollback must remove all audit rows");
    }

    #[tokio::test]
    async fn rollback_deletes_every_row_stamped_with_import_id() {
        let (_dir, pool, hh_id) = setup_db().await;
        let book = reader::parse(test_fixtures::banktivity_minimal()).unwrap();
        let plan = build_default_plan(hh_id.clone(), new_ulid(), &book, new_ulid).unwrap();
        commit(&pool, &plan, 0, 100).await.unwrap();

        rollback(&pool, &plan.import_id).await.unwrap();

        let (acc_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE import_id = ?")
                .bind(&plan.import_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(acc_count, 0);

        let (txn_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE import_id = ?")
                .bind(&plan.import_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(txn_count, 0);

        let (orphans,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM journal_lines jl LEFT JOIN transactions t ON t.id = jl.transaction_id WHERE t.id IS NULL",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(orphans, 0);
    }
}
