// Audit log writes (#143).
//
// Every ledger mutation (transaction insert, status change, journal-line
// write) must produce an audit_log row in the same SQL transaction as the
// data write. The audit_log table has INSERT-only triggers (migration 0001),
// so this is the one source of truth for "what changed and when" — load-
// bearing for the GAAP positioning.
//
// Failures here propagate. An audit row that points to a non-existent
// transaction is worse than no audit row, but a successful data write with
// a missing audit row is also unacceptable; both halves must commit together.

use serde::Serialize;
use sqlx::{Sqlite, Transaction};

use crate::id::new_ulid;

/// SQL action enum allowed by the `audit_log.action` CHECK constraint.
#[derive(Debug, Clone, Copy)]
pub enum AuditAction {
    Insert,
    Update,
    Delete,
}

impl AuditAction {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Insert => "insert",
            Self::Update => "update",
            Self::Delete => "delete",
        }
    }
}

/// Writes one audit_log row inside the caller's open SQL transaction.
/// `payload` is serialized to JSON and stored verbatim — full snapshots
/// are intentional (#143 design decision: disk is cheap, joins are not).
pub async fn write<P: Serialize>(
    tx: &mut Transaction<'_, Sqlite>,
    household_id: &str,
    table_name: &str,
    row_id: &str,
    action: AuditAction,
    payload: &P,
    now_ms: i64,
) -> Result<(), sqlx::Error> {
    let payload_json = serde_json::to_string(payload).map_err(|e| {
        // Bubble JSON errors up through sqlx::Error so the caller doesn't
        // need a separate audit error type.
        sqlx::Error::Encode(Box::new(e))
    })?;

    sqlx::query(
        "INSERT INTO audit_log (id, household_id, table_name, row_id, action, payload, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(new_ulid())
    .bind(household_id)
    .bind(table_name)
    .bind(row_id)
    .bind(action.as_str())
    .bind(&payload_json)
    .bind(now_ms)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_encrypted_db;
    use serde_json::json;
    use tempfile::tempdir;

    async fn pool_with_household() -> (sqlx::SqlitePool, String) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("audit.db");
        let pool = create_encrypted_db(&path, "pp", &[0u8; 16]).await.unwrap();
        let hh = new_ulid();
        sqlx::query(
            "INSERT INTO households (id, name, timezone, created_at) VALUES (?, 'H', 'UTC', 0)",
        )
        .bind(&hh)
        .execute(&pool)
        .await
        .unwrap();
        std::mem::forget(dir);
        (pool, hh)
    }

    #[tokio::test]
    async fn writes_row_with_serialized_payload() {
        let (pool, hh) = pool_with_household().await;
        let mut tx = pool.begin().await.unwrap();
        write(
            &mut tx,
            &hh,
            "transactions",
            "txn_001",
            AuditAction::Insert,
            &json!({ "memo": "Coffee", "amount_cents": 500 }),
            42,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let row: (String, String, String, String, i64) = sqlx::query_as(
            "SELECT table_name, row_id, action, payload, created_at FROM audit_log WHERE household_id = ?",
        )
        .bind(&hh)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.0, "transactions");
        assert_eq!(row.1, "txn_001");
        assert_eq!(row.2, "insert");
        assert!(row.3.contains("Coffee"));
        assert!(row.3.contains("500"));
        assert_eq!(row.4, 42);
    }

    #[tokio::test]
    async fn rolls_back_with_outer_transaction() {
        // If the caller's transaction aborts, the audit row must vanish too.
        let (pool, hh) = pool_with_household().await;
        let mut tx = pool.begin().await.unwrap();
        write(
            &mut tx,
            &hh,
            "transactions",
            "txn_002",
            AuditAction::Insert,
            &json!({ "anything": true }),
            0,
        )
        .await
        .unwrap();
        tx.rollback().await.unwrap();

        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM audit_log WHERE household_id = ? AND row_id = 'txn_002'",
        )
        .bind(&hh)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 0, "audit row must rollback with the outer transaction");
    }

    #[tokio::test]
    async fn supports_all_three_actions() {
        let (pool, hh) = pool_with_household().await;
        let mut tx = pool.begin().await.unwrap();
        for (i, action) in [AuditAction::Insert, AuditAction::Update, AuditAction::Delete]
            .iter()
            .enumerate()
        {
            write(
                &mut tx,
                &hh,
                "transactions",
                &format!("row_{i}"),
                *action,
                &json!({}),
                i as i64,
            )
            .await
            .unwrap();
        }
        tx.commit().await.unwrap();

        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit_log WHERE household_id = ?")
            .bind(&hh)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 3);
    }
}
