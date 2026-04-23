// Chat message persistence (T-045)
// Stores ChatMessage variants as JSON payloads keyed by household.
// The payload shape is owned by the frontend (`chatTypes.ts`); Rust is
// a pure append-and-read store that doesn't interpret kind-specific fields.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChatError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("unknown message kind: {0}")]
    UnknownKind(String),
}

/// Message kinds mirror the discriminated union in `chatTypes.ts`.
/// Kept in sync by the CHECK constraint on chat_messages.kind.
const VALID_KINDS: &[&str] = &[
    "user",
    "ai",
    "proactive",
    "system",
    "transaction",
    "artifact",
    "setup_card",
    "handoff",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageRow {
    pub id: String,
    pub kind: String,
    pub payload: String,
    pub ts: i64,
}

#[derive(Clone)]
pub struct ChatRepo {
    pool: SqlitePool,
}

impl ChatRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn append(
        &self,
        household_id: &str,
        id: &str,
        kind: &str,
        payload: &str,
        ts: i64,
        created_at: i64,
    ) -> Result<(), ChatError> {
        if !VALID_KINDS.contains(&kind) {
            return Err(ChatError::UnknownKind(kind.to_string()));
        }
        sqlx::query(
            "INSERT INTO chat_messages (id, household_id, kind, payload, ts, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(household_id)
        .bind(kind)
        .bind(payload)
        .bind(ts)
        .bind(created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Returns up to `limit` messages with ts < `before_ts`, newest first.
    /// Pass `i64::MAX` for `before_ts` to read the tail of the thread.
    pub async fn list_before(
        &self,
        household_id: &str,
        before_ts: i64,
        limit: i64,
    ) -> Result<Vec<ChatMessageRow>, ChatError> {
        let rows = sqlx::query_as::<_, (String, String, String, i64)>(
            "SELECT id, kind, payload, ts
             FROM chat_messages
             WHERE household_id = ? AND ts < ?
             ORDER BY ts DESC, created_at DESC
             LIMIT ?",
        )
        .bind(household_id)
        .bind(before_ts)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, kind, payload, ts)| ChatMessageRow { id, kind, payload, ts })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::create_encrypted_db;
    use crate::id::new_ulid;
    use tempfile::tempdir;

    async fn setup() -> (SqlitePool, String) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("chat.db");
        let pool = create_encrypted_db(&path, "pw", &[0u8; 16]).await.unwrap();
        let hid = new_ulid();
        sqlx::query(
            "INSERT INTO households (id, name, timezone, created_at) VALUES (?, 'H', 'UTC', 0)",
        )
        .bind(&hid)
        .execute(&pool)
        .await
        .unwrap();
        // Keep the tempdir alive by leaking; tests are short-lived.
        std::mem::forget(dir);
        (pool, hid)
    }

    #[tokio::test]
    async fn append_then_list_returns_the_message() {
        let (pool, hid) = setup().await;
        let repo = ChatRepo::new(pool);
        let id = new_ulid();

        repo.append(&hid, &id, "user", r#"{"kind":"user","text":"hi"}"#, 1000, 1000)
            .await
            .unwrap();

        let rows = repo.list_before(&hid, i64::MAX, 50).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, id);
        assert_eq!(rows[0].kind, "user");
        assert_eq!(rows[0].ts, 1000);
    }

    #[tokio::test]
    async fn list_orders_newest_first() {
        let (pool, hid) = setup().await;
        let repo = ChatRepo::new(pool);

        for (ts, id) in [(1000, new_ulid()), (3000, new_ulid()), (2000, new_ulid())] {
            repo.append(&hid, &id, "user", r#"{}"#, ts, ts).await.unwrap();
        }

        let rows = repo.list_before(&hid, i64::MAX, 10).await.unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].ts, 3000);
        assert_eq!(rows[1].ts, 2000);
        assert_eq!(rows[2].ts, 1000);
    }

    #[tokio::test]
    async fn list_before_paginates() {
        let (pool, hid) = setup().await;
        let repo = ChatRepo::new(pool);

        for ts in [1000, 2000, 3000, 4000, 5000_i64] {
            repo.append(&hid, &new_ulid(), "user", r#"{}"#, ts, ts)
                .await
                .unwrap();
        }

        // Tail: most recent 2
        let tail = repo.list_before(&hid, i64::MAX, 2).await.unwrap();
        let tail_ts: Vec<i64> = tail.iter().map(|r| r.ts).collect();
        assert_eq!(tail_ts, vec![5000, 4000]);

        // Older page before the oldest tail entry
        let older = repo.list_before(&hid, 4000, 2).await.unwrap();
        let older_ts: Vec<i64> = older.iter().map(|r| r.ts).collect();
        assert_eq!(older_ts, vec![3000, 2000]);
    }

    #[tokio::test]
    async fn list_is_scoped_to_household() {
        let (pool, hid_a) = setup().await;
        let hid_b = new_ulid();
        sqlx::query(
            "INSERT INTO households (id, name, timezone, created_at) VALUES (?, 'B', 'UTC', 0)",
        )
        .bind(&hid_b)
        .execute(&pool)
        .await
        .unwrap();

        let repo = ChatRepo::new(pool);
        repo.append(&hid_a, &new_ulid(), "user", r#"{}"#, 1000, 1000).await.unwrap();
        repo.append(&hid_b, &new_ulid(), "user", r#"{}"#, 2000, 2000).await.unwrap();

        let a_rows = repo.list_before(&hid_a, i64::MAX, 10).await.unwrap();
        assert_eq!(a_rows.len(), 1);
        assert_eq!(a_rows[0].ts, 1000);
    }

    #[tokio::test]
    async fn append_rejects_unknown_kind() {
        let (pool, hid) = setup().await;
        let repo = ChatRepo::new(pool);
        let err = repo
            .append(&hid, &new_ulid(), "garbage", r#"{}"#, 1000, 1000)
            .await
            .unwrap_err();
        assert!(matches!(err, ChatError::UnknownKind(_)));
    }

    #[tokio::test]
    async fn accepts_all_valid_kinds() {
        let (pool, hid) = setup().await;
        let repo = ChatRepo::new(pool);
        for kind in VALID_KINDS {
            repo.append(&hid, &new_ulid(), kind, r#"{}"#, 1000, 1000)
                .await
                .unwrap();
        }
        let rows = repo.list_before(&hid, i64::MAX, 100).await.unwrap();
        assert_eq!(rows.len(), VALID_KINDS.len());
    }
}
