//! Atomically commits an `ImportPlan` to the Tally database. One SQL
//! transaction wraps everything; any failure rolls back.

use super::{AccountType, ImportError, ImportPlan, ImportReceipt, NormalBalance, Side};
use sqlx::{Acquire, SqlitePool};

pub async fn commit(pool: &SqlitePool, plan: &ImportPlan, now_ms: i64) -> Result<ImportReceipt, ImportError> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;

    // Insert mapped accounts
    for m in &plan.account_mappings {
        sqlx::query(
            "INSERT INTO accounts (id, household_id, parent_id, name, type, normal_balance, is_placeholder, currency, created_at, import_id) \
             VALUES (?, ?, ?, ?, ?, ?, 0, 'USD', ?, ?)"
        )
        .bind(&m.tally_account_id)
        .bind(&plan.household_id)
        .bind(&m.tally_parent_id)
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
        // Idempotency: skip if (household_id, source_ref) already exists
        let exists: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM transactions WHERE household_id = ? AND source_ref = ?",
        )
        .bind(&plan.household_id)
        .bind(&ptx.gnc_guid)
        .fetch_one(&mut *tx)
        .await?;
        if exists.0 > 0 {
            skipped += 1;
            continue;
        }

        let txn_ulid = crate::id::new_ulid();
        sqlx::query(
            "INSERT INTO transactions (id, household_id, txn_date, entry_date, status, source, memo, import_id, source_ref, created_at) \
             VALUES (?, ?, ?, ?, 'posted', 'import', ?, ?, ?, ?)"
        )
        .bind(&txn_ulid)
        .bind(&plan.household_id)
        .bind(ptx.txn_date)
        .bind(now_ms)
        .bind(&ptx.memo)
        .bind(&plan.import_id)
        .bind(&ptx.gnc_guid)
        .bind(now_ms)
        .execute(&mut *tx)
        .await?;

        for line in &ptx.lines {
            let line_ulid = crate::id::new_ulid();
            sqlx::query(
                "INSERT INTO journal_lines (id, transaction_id, account_id, envelope_id, amount, side, created_at) \
                 VALUES (?, ?, ?, NULL, ?, ?, ?)"
            )
            .bind(&line_ulid)
            .bind(&txn_ulid)
            .bind(&line.tally_account_id)
            .bind(line.amount_cents)
            .bind(side_str(line.side))
            .bind(now_ms)
            .execute(&mut *tx)
            .await?;
        }

        committed += 1;
    }

    tx.commit().await?;

    Ok(ImportReceipt {
        import_id: plan.import_id.clone(),
        accounts_created: plan.account_mappings.len() as u32,
        transactions_committed: committed,
        transactions_skipped: skipped,
    })
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
    use super::super::test_fixtures::{build_fixture, happy_spec};
    use super::super::reader::read;
    use super::super::mapper::build_default_plan;
    use super::*;
    use crate::db::{connection::create_encrypted_db, migrations::run_migrations};
    use tempfile::tempdir;

    async fn setup_db() -> (tempfile::TempDir, SqlitePool, String) {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("tally.db");
        let salt = [0u8; 16];
        let pool = create_encrypted_db(&db_path, "pp", &salt).await.unwrap();
        run_migrations(&pool).await.unwrap();
        let hh_id = crate::id::new_ulid();
        sqlx::query("INSERT INTO households (id, name, timezone, created_at) VALUES (?, 'H', 'America/Chicago', 0)")
            .bind(&hh_id)
            .execute(&pool).await.unwrap();
        (dir, pool, hh_id)
    }

    #[tokio::test]
    async fn happy_path_commits_accounts_and_transactions() {
        let (_dir, pool, hh_id) = setup_db().await;
        let fixture_dir = tempdir().unwrap();
        let fixture_path = build_fixture(fixture_dir.path(), &happy_spec()).await;
        let book = read(&fixture_path).await.unwrap();
        let plan = build_default_plan(hh_id.clone(), crate::id::new_ulid(), &book, crate::id::new_ulid).unwrap();

        let receipt = commit(&pool, &plan, 100).await.unwrap();
        assert_eq!(receipt.accounts_created, 3);
        assert_eq!(receipt.transactions_committed, 2);
        assert_eq!(receipt.transactions_skipped, 0);

        let (acc_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE household_id = ?")
            .bind(&hh_id).fetch_one(&pool).await.unwrap();
        assert_eq!(acc_count, 3);

        let (txn_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE household_id = ? AND source = 'import'")
            .bind(&hh_id).fetch_one(&pool).await.unwrap();
        assert_eq!(txn_count, 2);
    }

    #[tokio::test]
    async fn running_same_plan_twice_skips_all_transactions_on_second_run() {
        let (_dir, pool, hh_id) = setup_db().await;
        let fixture_dir = tempdir().unwrap();
        let fixture_path = build_fixture(fixture_dir.path(), &happy_spec()).await;
        let book = read(&fixture_path).await.unwrap();
        let plan = build_default_plan(hh_id.clone(), crate::id::new_ulid(), &book, crate::id::new_ulid).unwrap();

        let first = commit(&pool, &plan, 100).await.unwrap();
        assert_eq!(first.transactions_committed, 2);

        // Second commit needs fresh account ulids, otherwise PK collision.
        let plan2 = build_default_plan(hh_id.clone(), crate::id::new_ulid(), &book, crate::id::new_ulid).unwrap();
        let second = commit(&pool, &plan2, 200).await.unwrap();
        assert_eq!(second.transactions_committed, 0);
        assert_eq!(second.transactions_skipped, 2);
    }

    #[tokio::test]
    async fn commit_rolls_back_entirely_when_any_row_fails() {
        let (_dir, pool, hh_id) = setup_db().await;
        let fixture_dir = tempdir().unwrap();
        let fixture_path = build_fixture(fixture_dir.path(), &happy_spec()).await;
        let book = read(&fixture_path).await.unwrap();
        let mut plan = build_default_plan(hh_id.clone(), crate::id::new_ulid(), &book, crate::id::new_ulid).unwrap();

        // Corrupt the plan: second transaction's first line references a nonexistent account ID.
        plan.transactions[1].lines[0].tally_account_id = "ULID_NONEXISTENT".into();

        let err = commit(&pool, &plan, 100).await.unwrap_err();
        assert!(matches!(err, ImportError::Database(_)));

        let (acc_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE household_id = ?")
            .bind(&hh_id).fetch_one(&pool).await.unwrap();
        assert_eq!(acc_count, 0, "accounts must be rolled back on failure");

        let (txn_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE household_id = ?")
            .bind(&hh_id).fetch_one(&pool).await.unwrap();
        assert_eq!(txn_count, 0, "transactions must be rolled back on failure");
    }
}
