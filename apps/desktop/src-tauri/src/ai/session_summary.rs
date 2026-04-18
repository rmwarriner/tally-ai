// Session summary compression — T-027
// Stores compressed session summaries; prunes entries older than 12 months on write.
// Writes are fire-and-forget (tokio::spawn) to never block the response path.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// 12 months expressed as milliseconds (365 days).
const TWELVE_MONTHS_MS: i64 = 365 * 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub household_id: String,
    pub session_id: String,
    pub summary_text: String,
    pub created_at_ms: i64,
}

/// Stores a compressed summary and prunes entries outside the 12-month window.
/// Returns after DB operations complete.
pub async fn store_summary(
    pool: &SqlitePool,
    household_id: &str,
    session_id: &str,
    summary_text: &str,
    now_ms: i64,
) -> Result<(), sqlx::Error> {
    let id = crate::id::new_ulid();
    sqlx::query(
        "INSERT INTO session_summaries (id, household_id, session_id, summary_text, created_at_ms)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(household_id)
    .bind(session_id)
    .bind(summary_text)
    .bind(now_ms)
    .execute(pool)
    .await?;

    prune_old_summaries(pool, household_id, now_ms).await?;
    Ok(())
}

/// Fire-and-forget version: spawns a Tokio task; does not block the response path.
pub fn store_summary_async(
    pool: SqlitePool,
    household_id: String,
    session_id: String,
    summary_text: String,
    now_ms: i64,
) {
    tokio::spawn(async move {
        let _ = store_summary(&pool, &household_id, &session_id, &summary_text, now_ms).await;
    });
}

/// Returns all summaries for the household within the 12-month window, oldest first.
pub async fn get_summaries(
    pool: &SqlitePool,
    household_id: &str,
    now_ms: i64,
) -> Result<Vec<SessionSummary>, sqlx::Error> {
    let cutoff = now_ms - TWELVE_MONTHS_MS;
    sqlx::query_as::<_, SummaryRow>(
        "SELECT id, household_id, session_id, summary_text, created_at_ms
         FROM session_summaries
         WHERE household_id = ?
           AND created_at_ms >= ?
         ORDER BY created_at_ms ASC",
    )
    .bind(household_id)
    .bind(cutoff)
    .fetch_all(pool)
    .await
    .map(|rows| rows.into_iter().map(SummaryRow::into_summary).collect())
}

async fn prune_old_summaries(
    pool: &SqlitePool,
    household_id: &str,
    now_ms: i64,
) -> Result<(), sqlx::Error> {
    let cutoff = now_ms - TWELVE_MONTHS_MS;
    sqlx::query(
        "DELETE FROM session_summaries WHERE household_id = ? AND created_at_ms < ?",
    )
    .bind(household_id)
    .bind(cutoff)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct SummaryRow {
    id: String,
    household_id: String,
    session_id: String,
    summary_text: String,
    created_at_ms: i64,
}

impl SummaryRow {
    fn into_summary(self) -> SessionSummary {
        SessionSummary {
            id: self.id,
            household_id: self.household_id,
            session_id: self.session_id,
            summary_text: self.summary_text,
            created_at_ms: self.created_at_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::create_encrypted_db;
    use crate::db::migrations::run_migrations;
    use crate::id::new_ulid;
    use tempfile::tempdir;

    async fn setup() -> (SqlitePool, String) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("summary_test.db");
        let pool = create_encrypted_db(&path, "pw", &[0u8; 16]).await.unwrap();
        run_migrations(&pool).await.unwrap();

        let hid = new_ulid();
        sqlx::query(
            "INSERT INTO households (id, name, timezone, created_at) VALUES (?, 'Test', 'UTC', 0)",
        )
        .bind(&hid)
        .execute(&pool)
        .await
        .unwrap();

        (pool, hid)
    }

    #[tokio::test]
    async fn stores_and_retrieves_summary() {
        let (pool, hid) = setup().await;
        let sid = new_ulid();
        let now = 1_000_000_000_000_i64;

        store_summary(&pool, &hid, &sid, "session recap", now).await.unwrap();

        let summaries = get_summaries(&pool, &hid, now).await.unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].summary_text, "session recap");
        assert_eq!(summaries[0].session_id, sid);
    }

    #[tokio::test]
    async fn retrieval_excludes_other_households() {
        let (pool, hid) = setup().await;

        let hid2 = new_ulid();
        sqlx::query(
            "INSERT INTO households (id, name, timezone, created_at) VALUES (?, 'Other', 'UTC', 0)",
        )
        .bind(&hid2)
        .execute(&pool)
        .await
        .unwrap();

        let now = 1_000_000_000_000_i64;
        store_summary(&pool, &hid, &new_ulid(), "mine", now).await.unwrap();
        store_summary(&pool, &hid2, &new_ulid(), "theirs", now).await.unwrap();

        let summaries = get_summaries(&pool, &hid, now).await.unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].summary_text, "mine");
    }

    #[tokio::test]
    async fn prunes_entries_older_than_12_months() {
        let (pool, hid) = setup().await;

        let now = 1_000_000_000_000_i64;
        let old = now - TWELVE_MONTHS_MS - 1;

        store_summary(&pool, &hid, &new_ulid(), "old recap", old).await.unwrap();
        // Write a recent summary to trigger pruning.
        store_summary(&pool, &hid, &new_ulid(), "recent recap", now).await.unwrap();

        let summaries = get_summaries(&pool, &hid, now).await.unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].summary_text, "recent recap");
    }

    #[tokio::test]
    async fn summaries_returned_oldest_first() {
        let (pool, hid) = setup().await;

        let now = 1_000_000_000_000_i64;
        let earlier = now - 60_000;

        store_summary(&pool, &hid, &new_ulid(), "first", earlier).await.unwrap();
        store_summary(&pool, &hid, &new_ulid(), "second", now).await.unwrap();

        let summaries = get_summaries(&pool, &hid, now).await.unwrap();
        assert_eq!(summaries[0].summary_text, "first");
        assert_eq!(summaries[1].summary_text, "second");
    }

    #[tokio::test]
    async fn empty_result_when_no_summaries() {
        let (pool, hid) = setup().await;
        let summaries = get_summaries(&pool, &hid, 1_000_000_000_000).await.unwrap();
        assert!(summaries.is_empty());
    }

    #[tokio::test]
    async fn boundary_entry_at_exactly_12_months_is_included() {
        let (pool, hid) = setup().await;

        let now = 1_000_000_000_000_i64;
        let boundary = now - TWELVE_MONTHS_MS;

        // Insert directly to bypass store_summary's pruning logic.
        let id = new_ulid();
        sqlx::query(
            "INSERT INTO session_summaries (id, household_id, session_id, summary_text, created_at_ms)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&hid)
        .bind(&new_ulid())
        .bind("boundary")
        .bind(boundary)
        .execute(&pool)
        .await
        .unwrap();

        // get_summaries uses created_at_ms >= cutoff, so the boundary row is included.
        let summaries = get_summaries(&pool, &hid, now).await.unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].summary_text, "boundary");
    }
}
