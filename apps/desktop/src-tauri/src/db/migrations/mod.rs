use sqlx::SqlitePool;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MigrationError {
    #[error("Migration failed: {0}")]
    Failed(#[from] sqlx::migrate::MigrateError),
}

/// Runs all pending migrations against the provided pool.
///
/// Migrations are embedded at compile time from the `migrations/` directory
/// adjacent to Cargo.toml. Never edit or delete past migration files.
pub async fn run_migrations(pool: &SqlitePool) -> Result<(), MigrationError> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::create_encrypted_db;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_migrations_create_schema() {
        let dir = tempdir().expect("Should create temp dir");
        let db_path = dir.path().join("test_migration.db");
        let salt = [0u8; 16];

        let pool = create_encrypted_db(&db_path, "passphrase", &salt)
            .await
            .expect("Should create database");

        // Verify all Phase 1 tables exist
        let tables: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
                .fetch_all(&pool)
                .await
                .expect("Should query tables");

        let table_names: Vec<&str> = tables.iter().map(|(n,)| n.as_str()).collect();

        // Core identity tables
        assert!(table_names.contains(&"households"), "Missing households table");
        assert!(table_names.contains(&"users"), "Missing users table");

        // Chart of accounts
        assert!(table_names.contains(&"accounts"), "Missing accounts table");

        // Transactions and journal lines
        assert!(table_names.contains(&"transactions"), "Missing transactions table");
        assert!(
            table_names.contains(&"journal_lines"),
            "Missing journal_lines table"
        );

        // Envelopes
        assert!(table_names.contains(&"envelopes"), "Missing envelopes table");
        assert!(
            table_names.contains(&"envelope_periods"),
            "Missing envelope_periods table"
        );

        // Audit log
        assert!(table_names.contains(&"audit_log"), "Missing audit_log table");

        // AI layer
        assert!(table_names.contains(&"payee_memory"), "Missing payee_memory table");
        assert!(table_names.contains(&"session_summaries"), "Missing session_summaries table");

        // Verify no deprecated tables
        assert!(
            !table_names.contains(&"journal_entries"),
            "journal_entries should be transactions"
        );
    }

    #[tokio::test]
    async fn test_households_timezone_required() {
        let dir = tempdir().expect("Should create temp dir");
        let db_path = dir.path().join("test_timezone.db");
        let salt = [0u8; 16];

        let pool = create_encrypted_db(&db_path, "passphrase", &salt)
            .await
            .expect("Should create database");

        run_migrations(&pool)
            .await
            .expect("Migrations should run");

        // Verify timezone column is NOT NULL by checking schema
        let schema: Vec<(String,)> = sqlx::query_as(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='households'",
        )
        .fetch_all(&pool)
        .await
        .expect("Should query table schema");

        let table_def = schema.first().expect("households table should exist");
        assert!(
            table_def.0.contains("timezone       TEXT NOT NULL"),
            "timezone should be NOT NULL"
        );
    }

    #[tokio::test]
    async fn test_money_stored_as_integers() {
        let dir = tempdir().expect("Should create temp dir");
        let db_path = dir.path().join("test_money.db");
        let salt = [0u8; 16];

        let pool = create_encrypted_db(&db_path, "passphrase", &salt)
            .await
            .expect("Should create database");

        run_migrations(&pool)
            .await
            .expect("Migrations should run");

        // Verify amount column is INTEGER by checking the schema
        let schema: Vec<(String,)> = sqlx::query_as(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='journal_lines'",
        )
        .fetch_all(&pool)
        .await
        .expect("Should query table schema");

        let table_def = schema.first().expect("journal_lines table should exist");
        assert!(
            table_def.0.contains("amount") && table_def.0.contains("INTEGER"),
            "amount should be INTEGER, not REAL"
        );
        assert!(
            !table_def.0.contains("REAL"),
            "schema should not use REAL for monetary amounts"
        );
    }

    #[tokio::test]
    async fn test_migrations_idempotent() {
        let dir = tempdir().expect("Should create temp dir");
        let db_path = dir.path().join("test_idempotent.db");
        let salt = [0u8; 16];

        let pool = create_encrypted_db(&db_path, "passphrase", &salt)
            .await
            .expect("Should create database");

        // Running migrations again on the same pool should be a no-op
        run_migrations(&pool)
            .await
            .expect("Re-running migrations should succeed");
    }

    #[tokio::test]
    async fn test_audit_log_immutable_prevents_update() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let dir = tempdir().expect("Should create temp dir");
        let db_path = dir.path().join("test_audit_immutable.db");
        let salt = [0u8; 16];

        let pool = create_encrypted_db(&db_path, "passphrase", &salt)
            .await
            .expect("Should create database");

        run_migrations(&pool)
            .await
            .expect("Migrations should run");

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // Insert a household and audit log entry
        sqlx::query(
            "INSERT INTO households (id, name, timezone, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind("household_id")
        .bind("Test Household")
        .bind("America/Chicago")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert household");

        sqlx::query(
            "INSERT INTO audit_log (id, household_id, table_name, row_id, action, payload, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("audit_1")
        .bind("household_id")
        .bind("households")
        .bind("household_id")
        .bind("insert")
        .bind("{\"name\":\"Test\"}")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert audit log entry");

        // Attempt UPDATE should fail
        let update_result = sqlx::query("UPDATE audit_log SET payload = ? WHERE id = ?")
            .bind("{\"name\":\"Updated\"}")
            .bind("audit_1")
            .execute(&pool)
            .await;

        assert!(
            update_result.is_err(),
            "UPDATE on audit_log should fail due to trigger"
        );
    }

    #[tokio::test]
    async fn test_audit_log_immutable_prevents_delete() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let dir = tempdir().expect("Should create temp dir");
        let db_path = dir.path().join("test_audit_delete.db");
        let salt = [0u8; 16];

        let pool = create_encrypted_db(&db_path, "passphrase", &salt)
            .await
            .expect("Should create database");

        run_migrations(&pool)
            .await
            .expect("Migrations should run");

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // Insert a household and audit log entry
        sqlx::query(
            "INSERT INTO households (id, name, timezone, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind("household_id")
        .bind("Test Household")
        .bind("America/Chicago")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert household");

        sqlx::query(
            "INSERT INTO audit_log (id, household_id, table_name, row_id, action, payload, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("audit_1")
        .bind("household_id")
        .bind("households")
        .bind("household_id")
        .bind("insert")
        .bind("{\"name\":\"Test\"}")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert audit log entry");

        // Attempt DELETE should fail
        let delete_result = sqlx::query("DELETE FROM audit_log WHERE id = ?")
            .bind("audit_1")
            .execute(&pool)
            .await;

        assert!(
            delete_result.is_err(),
            "DELETE on audit_log should fail due to trigger"
        );
    }

    #[tokio::test]
    async fn test_envelope_spent_increases_on_journal_insert() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let dir = tempdir().expect("Should create temp dir");
        let db_path = dir.path().join("test_envelope_insert.db");
        let salt = [0u8; 16];

        let pool = create_encrypted_db(&db_path, "passphrase", &salt)
            .await
            .expect("Should create database");

        run_migrations(&pool)
            .await
            .expect("Migrations should run");

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // Set up: household, account, envelope, envelope_period
        sqlx::query(
            "INSERT INTO households (id, name, timezone, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind("h1")
        .bind("Test Household")
        .bind("America/Chicago")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert household");

        sqlx::query(
            "INSERT INTO accounts (id, household_id, name, type, normal_balance, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("acc1")
        .bind("h1")
        .bind("Groceries")
        .bind("expense")
        .bind("debit")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert account");

        sqlx::query(
            "INSERT INTO envelopes (id, household_id, account_id, name, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind("env1")
        .bind("h1")
        .bind("acc1")
        .bind("Groceries Budget")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert envelope");

        let period_start = now - (now % 86400000); // Midnight UTC
        let period_end = period_start + 2592000000; // 30 days

        sqlx::query(
            "INSERT INTO envelope_periods (id, envelope_id, period_start, period_end, allocated, spent, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("ep1")
        .bind("env1")
        .bind(period_start)
        .bind(period_end)
        .bind(50000) // $500.00
        .bind(0)
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert envelope_period");

        // Create a posted transaction
        sqlx::query(
            "INSERT INTO transactions (id, household_id, txn_date, entry_date, status, source, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("txn1")
        .bind("h1")
        .bind(period_start)
        .bind(now)
        .bind("posted")
        .bind("manual")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert transaction");

        // Insert journal line with envelope_id
        sqlx::query(
            "INSERT INTO journal_lines (id, transaction_id, account_id, envelope_id, amount, side, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("jl1")
        .bind("txn1")
        .bind("acc1")
        .bind("env1")
        .bind(12000) // $120.00
        .bind("debit")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert journal line");

        // Verify spent increased
        let (spent,): (i64,) = sqlx::query_as("SELECT spent FROM envelope_periods WHERE id = ?")
            .bind("ep1")
            .fetch_one(&pool)
            .await
            .expect("Should fetch envelope_period");

        assert_eq!(spent, 12000, "spent should increase by journal line amount");
    }

    #[tokio::test]
    async fn test_envelope_spent_recalculates_on_journal_update() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let dir = tempdir().expect("Should create temp dir");
        let db_path = dir.path().join("test_envelope_update.db");
        let salt = [0u8; 16];

        let pool = create_encrypted_db(&db_path, "passphrase", &salt)
            .await
            .expect("Should create database");

        run_migrations(&pool)
            .await
            .expect("Migrations should run");

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // Set up household, account, envelope, period
        sqlx::query(
            "INSERT INTO households (id, name, timezone, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind("h2")
        .bind("Test Household")
        .bind("America/Chicago")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert household");

        sqlx::query(
            "INSERT INTO accounts (id, household_id, name, type, normal_balance, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("acc2")
        .bind("h2")
        .bind("Gas")
        .bind("expense")
        .bind("debit")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert account");

        sqlx::query(
            "INSERT INTO envelopes (id, household_id, account_id, name, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind("env2")
        .bind("h2")
        .bind("acc2")
        .bind("Gas Budget")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert envelope");

        let period_start = now - (now % 86400000);
        let period_end = period_start + 2592000000;

        sqlx::query(
            "INSERT INTO envelope_periods (id, envelope_id, period_start, period_end, allocated, spent, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("ep2")
        .bind("env2")
        .bind(period_start)
        .bind(period_end)
        .bind(10000)
        .bind(0)
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert envelope_period");

        sqlx::query(
            "INSERT INTO transactions (id, household_id, txn_date, entry_date, status, source, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("txn2")
        .bind("h2")
        .bind(period_start)
        .bind(now)
        .bind("posted")
        .bind("manual")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert transaction");

        sqlx::query(
            "INSERT INTO journal_lines (id, transaction_id, account_id, envelope_id, amount, side, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("jl2")
        .bind("txn2")
        .bind("acc2")
        .bind("env2")
        .bind(3000) // $30.00
        .bind("debit")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert journal line");

        // Update the journal line amount
        sqlx::query("UPDATE journal_lines SET amount = ? WHERE id = ?")
            .bind(5000) // $50.00
            .bind("jl2")
            .execute(&pool)
            .await
            .expect("Should update journal line");

        // Verify spent recalculated: should be 5000 not 3000
        let (spent,): (i64,) = sqlx::query_as("SELECT spent FROM envelope_periods WHERE id = ?")
            .bind("ep2")
            .fetch_one(&pool)
            .await
            .expect("Should fetch envelope_period");

        assert_eq!(
            spent, 5000,
            "spent should recalculate: subtract old amount, add new amount"
        );
    }

    #[tokio::test]
    async fn test_envelope_spent_decreases_on_journal_delete() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let dir = tempdir().expect("Should create temp dir");
        let db_path = dir.path().join("test_envelope_delete.db");
        let salt = [0u8; 16];

        let pool = create_encrypted_db(&db_path, "passphrase", &salt)
            .await
            .expect("Should create database");

        run_migrations(&pool)
            .await
            .expect("Migrations should run");

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // Set up
        sqlx::query(
            "INSERT INTO households (id, name, timezone, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind("h3")
        .bind("Test Household")
        .bind("America/Chicago")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert household");

        sqlx::query(
            "INSERT INTO accounts (id, household_id, name, type, normal_balance, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("acc3")
        .bind("h3")
        .bind("Entertainment")
        .bind("expense")
        .bind("debit")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert account");

        sqlx::query(
            "INSERT INTO envelopes (id, household_id, account_id, name, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind("env3")
        .bind("h3")
        .bind("acc3")
        .bind("Entertainment Budget")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert envelope");

        let period_start = now - (now % 86400000);
        let period_end = period_start + 2592000000;

        sqlx::query(
            "INSERT INTO envelope_periods (id, envelope_id, period_start, period_end, allocated, spent, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("ep3")
        .bind("env3")
        .bind(period_start)
        .bind(period_end)
        .bind(20000)
        .bind(8000) // Pre-populate spent
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert envelope_period");

        sqlx::query(
            "INSERT INTO transactions (id, household_id, txn_date, entry_date, status, source, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("txn3")
        .bind("h3")
        .bind(period_start)
        .bind(now)
        .bind("posted")
        .bind("manual")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert transaction");

        sqlx::query(
            "INSERT INTO journal_lines (id, transaction_id, account_id, envelope_id, amount, side, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("jl3")
        .bind("txn3")
        .bind("acc3")
        .bind("env3")
        .bind(5000) // $50.00
        .bind("debit")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert journal line");

        // Verify spent increased
        let (spent_before,): (i64,) =
            sqlx::query_as("SELECT spent FROM envelope_periods WHERE id = ?")
                .bind("ep3")
                .fetch_one(&pool)
                .await
                .expect("Should fetch envelope_period");

        assert_eq!(spent_before, 13000, "spent should be 8000 + 5000");

        // Delete the journal line
        sqlx::query("DELETE FROM journal_lines WHERE id = ?")
            .bind("jl3")
            .execute(&pool)
            .await
            .expect("Should delete journal line");

        // Verify spent decreased
        let (spent_after,): (i64,) =
            sqlx::query_as("SELECT spent FROM envelope_periods WHERE id = ?")
                .bind("ep3")
                .fetch_one(&pool)
                .await
                .expect("Should fetch envelope_period");

        assert_eq!(spent_after, 8000, "spent should decrease back to original");
    }

    #[tokio::test]
    async fn test_envelope_spent_only_updates_matching_period() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let dir = tempdir().expect("Should create temp dir");
        let db_path = dir.path().join("test_envelope_period_match.db");
        let salt = [0u8; 16];

        let pool = create_encrypted_db(&db_path, "passphrase", &salt)
            .await
            .expect("Should create database");

        run_migrations(&pool)
            .await
            .expect("Migrations should run");

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // Set up
        sqlx::query(
            "INSERT INTO households (id, name, timezone, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind("h4")
        .bind("Test Household")
        .bind("America/Chicago")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert household");

        sqlx::query(
            "INSERT INTO accounts (id, household_id, name, type, normal_balance, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("acc4")
        .bind("h4")
        .bind("Utilities")
        .bind("expense")
        .bind("debit")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert account");

        sqlx::query(
            "INSERT INTO envelopes (id, household_id, account_id, name, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind("env4")
        .bind("h4")
        .bind("acc4")
        .bind("Utilities Budget")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert envelope");

        let period1_start = now - (now % 86400000);
        let period1_end = period1_start + 2592000000; // 30 days
        let period2_start = period1_end + 86400000; // Next day after period 1 ends
        let period2_end = period2_start + 2592000000;

        // Create two envelope periods
        sqlx::query(
            "INSERT INTO envelope_periods (id, envelope_id, period_start, period_end, allocated, spent, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("ep4a")
        .bind("env4")
        .bind(period1_start)
        .bind(period1_end)
        .bind(10000)
        .bind(0)
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert first envelope_period");

        sqlx::query(
            "INSERT INTO envelope_periods (id, envelope_id, period_start, period_end, allocated, spent, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("ep4b")
        .bind("env4")
        .bind(period2_start)
        .bind(period2_end)
        .bind(10000)
        .bind(0)
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert second envelope_period");

        // Transaction well within period 2
        sqlx::query(
            "INSERT INTO transactions (id, household_id, txn_date, entry_date, status, source, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("txn4")
        .bind("h4")
        .bind(period2_start + 86400000)
        .bind(now)
        .bind("posted")
        .bind("manual")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert transaction");

        sqlx::query(
            "INSERT INTO journal_lines (id, transaction_id, account_id, envelope_id, amount, side, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("jl4")
        .bind("txn4")
        .bind("acc4")
        .bind("env4")
        .bind(3000)
        .bind("debit")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert journal line");

        // Verify only period2 was updated
        let (spent1,): (i64,) =
            sqlx::query_as("SELECT spent FROM envelope_periods WHERE id = ?")
                .bind("ep4a")
                .fetch_one(&pool)
                .await
                .expect("Should fetch period 1");

        let (spent2,): (i64,) =
            sqlx::query_as("SELECT spent FROM envelope_periods WHERE id = ?")
                .bind("ep4b")
                .fetch_one(&pool)
                .await
                .expect("Should fetch period 2");

        assert_eq!(spent1, 0, "period 1 should not be updated");
        assert_eq!(spent2, 3000, "period 2 should be updated");
    }

    #[tokio::test]
    async fn test_envelope_spent_decreases_on_credit_refund() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let dir = tempdir().expect("Should create temp dir");
        let db_path = dir.path().join("test_envelope_credit.db");
        let salt = [0u8; 16];

        let pool = create_encrypted_db(&db_path, "passphrase", &salt)
            .await
            .expect("Should create database");

        run_migrations(&pool).await.expect("Migrations should run");

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        sqlx::query(
            "INSERT INTO households (id, name, timezone, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind("h6")
        .bind("Test Household")
        .bind("America/Chicago")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert household");

        sqlx::query(
            "INSERT INTO accounts (id, household_id, name, type, normal_balance, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("acc6")
        .bind("h6")
        .bind("Groceries")
        .bind("expense")
        .bind("debit")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert account");

        sqlx::query(
            "INSERT INTO envelopes (id, household_id, account_id, name, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind("env6")
        .bind("h6")
        .bind("acc6")
        .bind("Groceries Budget")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert envelope");

        let period_start = now - (now % 86400000);
        let period_end = period_start + 2592000000;

        // Pre-populate spent with prior spending
        sqlx::query(
            "INSERT INTO envelope_periods (id, envelope_id, period_start, period_end, allocated, spent, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("ep6")
        .bind("env6")
        .bind(period_start)
        .bind(period_end)
        .bind(50000)
        .bind(12000) // $120.00 already spent
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert envelope_period");

        sqlx::query(
            "INSERT INTO transactions (id, household_id, txn_date, entry_date, status, source, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("txn6")
        .bind("h6")
        .bind(period_start)
        .bind(now)
        .bind("posted")
        .bind("manual")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert transaction");

        // Credit line = refund to expense account
        sqlx::query(
            "INSERT INTO journal_lines (id, transaction_id, account_id, envelope_id, amount, side, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("jl6")
        .bind("txn6")
        .bind("acc6")
        .bind("env6")
        .bind(5000) // $50.00 refund
        .bind("credit")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert credit journal line");

        let (spent,): (i64,) = sqlx::query_as("SELECT spent FROM envelope_periods WHERE id = ?")
            .bind("ep6")
            .fetch_one(&pool)
            .await
            .expect("Should fetch envelope_period");

        assert_eq!(spent, 7000, "credit (refund) should decrease spent: 12000 - 5000 = 7000");
    }

    #[tokio::test]
    async fn test_migration_0006_adds_source_ref_and_import_id() {
        let dir = tempdir().expect("Should create temp dir");
        let db_path = dir.path().join("test_0006.db");
        let salt = [0u8; 16];

        let pool = create_encrypted_db(&db_path, "passphrase", &salt)
            .await
            .expect("Should create database");

        run_migrations(&pool).await.expect("Migrations should run");

        // transactions.source_ref exists
        let txn_schema: (String,) = sqlx::query_as(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='transactions'",
        )
        .fetch_one(&pool)
        .await
        .expect("transactions table should exist");
        assert!(
            txn_schema.0.contains("source_ref"),
            "transactions.source_ref column missing"
        );

        // accounts.import_id exists
        let acc_schema: (String,) = sqlx::query_as(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='accounts'",
        )
        .fetch_one(&pool)
        .await
        .expect("accounts table should exist");
        assert!(
            acc_schema.0.contains("import_id"),
            "accounts.import_id column missing"
        );

        // Unique (household_id, source_ref) index exists
        let indexes: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='transactions'",
        )
        .fetch_all(&pool)
        .await
        .expect("Should query indexes");
        let names: Vec<&str> = indexes.iter().map(|(n,)| n.as_str()).collect();
        assert!(
            names.contains(&"idx_transactions_source_ref_unique"),
            "missing idx_transactions_source_ref_unique"
        );

        // Lock in the critical properties of the idempotency index
        let unique_idx_sql: (String,) = sqlx::query_as(
            "SELECT sql FROM sqlite_master WHERE type='index' AND name='idx_transactions_source_ref_unique'",
        )
        .fetch_one(&pool)
        .await
        .expect("unique index should exist");
        assert!(
            unique_idx_sql.0.contains("UNIQUE"),
            "idempotency index must be UNIQUE"
        );
        assert!(
            unique_idx_sql.0.contains("source_ref IS NOT NULL"),
            "index must be partial (WHERE source_ref IS NOT NULL) to allow multiple NULL source_refs per household"
        );
    }

    #[tokio::test]
    async fn test_envelope_spent_ignores_pending_transactions() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let dir = tempdir().expect("Should create temp dir");
        let db_path = dir.path().join("test_envelope_pending.db");
        let salt = [0u8; 16];

        let pool = create_encrypted_db(&db_path, "passphrase", &salt)
            .await
            .expect("Should create database");

        run_migrations(&pool)
            .await
            .expect("Migrations should run");

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // Set up
        sqlx::query(
            "INSERT INTO households (id, name, timezone, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind("h5")
        .bind("Test Household")
        .bind("America/Chicago")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert household");

        sqlx::query(
            "INSERT INTO accounts (id, household_id, name, type, normal_balance, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("acc5")
        .bind("h5")
        .bind("Groceries")
        .bind("expense")
        .bind("debit")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert account");

        sqlx::query(
            "INSERT INTO envelopes (id, household_id, account_id, name, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind("env5")
        .bind("h5")
        .bind("acc5")
        .bind("Groceries Budget")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert envelope");

        let period_start = now - (now % 86400000);
        let period_end = period_start + 2592000000;

        sqlx::query(
            "INSERT INTO envelope_periods (id, envelope_id, period_start, period_end, allocated, spent, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("ep5")
        .bind("env5")
        .bind(period_start)
        .bind(period_end)
        .bind(50000)
        .bind(0)
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert envelope_period");

        // Create PENDING transaction (not posted)
        sqlx::query(
            "INSERT INTO transactions (id, household_id, txn_date, entry_date, status, source, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("txn5")
        .bind("h5")
        .bind(period_start)
        .bind(now)
        .bind("pending")
        .bind("manual")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert transaction");

        sqlx::query(
            "INSERT INTO journal_lines (id, transaction_id, account_id, envelope_id, amount, side, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("jl5")
        .bind("txn5")
        .bind("acc5")
        .bind("env5")
        .bind(12000)
        .bind("debit")
        .bind(now)
        .execute(&pool)
        .await
        .expect("Should insert journal line");

        // Verify spent remains 0 (trigger only fires on posted status)
        let (spent,): (i64,) =
            sqlx::query_as("SELECT spent FROM envelope_periods WHERE id = ?")
                .bind("ep5")
                .fetch_one(&pool)
                .await
                .expect("Should fetch envelope_period");

        assert_eq!(spent, 0, "spent should not increase for pending transactions");
    }
}
