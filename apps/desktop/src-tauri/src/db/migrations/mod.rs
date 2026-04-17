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

        // Verify core tables exist
        let tables: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
                .fetch_all(&pool)
                .await
                .expect("Should query tables");

        let table_names: Vec<&str> = tables.iter().map(|(n,)| n.as_str()).collect();

        assert!(table_names.contains(&"accounts"), "Missing accounts table");
        assert!(table_names.contains(&"audit_log"), "Missing audit_log table");
        assert!(table_names.contains(&"envelopes"), "Missing envelopes table");
        assert!(table_names.contains(&"households"), "Missing households table");
        assert!(
            table_names.contains(&"journal_entries"),
            "Missing journal_entries table"
        );
        assert!(
            table_names.contains(&"journal_lines"),
            "Missing journal_lines table"
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
}
