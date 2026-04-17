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
}
