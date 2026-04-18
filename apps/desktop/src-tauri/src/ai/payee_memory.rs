// Payee memory — T-024
// Household-scoped payee → account mappings backed by the payee_memory table.
// An in-process LRU cache (500 entries) sits in front of the DB.
// Writes are async fire-and-forget to never block the response path.

use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use lru::LruCache;
use sqlx::SqlitePool;

use crate::id::new_ulid;

const CACHE_CAPACITY: usize = 500;

/// A single payee → account mapping returned by lookup or top_hints.
#[derive(Debug, Clone)]
pub struct MemoryHint {
    pub payee_name: String,
    pub account_id: String,
    pub use_count: u32,
}

/// Cache key: `"{household_id}:{lower(payee_name)}"`.
type CacheKey = String;

fn cache_key(household_id: &str, payee_name: &str) -> CacheKey {
    format!("{}:{}", household_id, payee_name.to_lowercase())
}

#[derive(Clone)]
pub struct PayeeMemory {
    pool: SqlitePool,
    cache: Arc<Mutex<LruCache<CacheKey, MemoryHint>>>,
}

impl PayeeMemory {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            cache: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(CACHE_CAPACITY).unwrap(),
            ))),
        }
    }

    /// Look up a payee mapping. Checks the LRU cache first, then the DB.
    pub async fn lookup(
        &self,
        household_id: &str,
        payee_name: &str,
    ) -> Option<MemoryHint> {
        let key = cache_key(household_id, payee_name);

        // Cache hit.
        if let Ok(mut cache) = self.cache.lock() {
            if let Some(hint) = cache.get(&key) {
                return Some(hint.clone());
            }
        }

        // DB lookup.
        let row = sqlx::query_as::<_, (String, String, i64)>(
            "SELECT payee_name, account_id, use_count
             FROM payee_memory
             WHERE household_id = ? AND payee_name = ? COLLATE NOCASE
             LIMIT 1",
        )
        .bind(household_id)
        .bind(payee_name)
        .fetch_optional(&self.pool)
        .await
        .ok()??;

        let hint = MemoryHint {
            payee_name: row.0,
            account_id: row.1,
            use_count: row.2 as u32,
        };

        if let Ok(mut cache) = self.cache.lock() {
            cache.put(key, hint.clone());
        }

        Some(hint)
    }

    /// Persist a payee mapping synchronously (use in tests and explicit callers).
    pub async fn record(
        &self,
        household_id: &str,
        payee_name: &str,
        account_id: &str,
    ) -> Result<(), sqlx::Error> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        sqlx::query(
            r#"
            INSERT INTO payee_memory (id, household_id, payee_name, account_id, use_count, last_used_ms, created_at)
            VALUES (?, ?, ?, ?, 1, ?, ?)
            ON CONFLICT(household_id, payee_name) DO UPDATE SET
                account_id   = excluded.account_id,
                use_count    = payee_memory.use_count + 1,
                last_used_ms = excluded.last_used_ms
            "#,
        )
        .bind(new_ulid())
        .bind(household_id)
        .bind(payee_name)
        .bind(account_id)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        // Refresh cache entry.
        let key = cache_key(household_id, payee_name);
        let updated_count = sqlx::query_as::<_, (i64,)>(
            "SELECT use_count FROM payee_memory WHERE household_id = ? AND payee_name = ? COLLATE NOCASE",
        )
        .bind(household_id)
        .bind(payee_name)
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten()
        .map(|(c,)| c as u32)
        .unwrap_or(1);

        if let Ok(mut cache) = self.cache.lock() {
            cache.put(key, MemoryHint {
                payee_name: payee_name.to_string(),
                account_id: account_id.to_string(),
                use_count: updated_count,
            });
        }

        Ok(())
    }

    /// Fire-and-forget version of `record`. Never blocks the response path.
    pub fn record_async(&self, household_id: String, payee_name: String, account_id: String) {
        let this = self.clone();
        tokio::spawn(async move {
            let _ = this.record(&household_id, &payee_name, &account_id).await;
        });
    }

    /// Returns the N most-frequently-used payee mappings for a household.
    pub async fn top_hints(
        &self,
        household_id: &str,
        n: usize,
    ) -> Vec<MemoryHint> {
        sqlx::query_as::<_, (String, String, i64)>(
            "SELECT payee_name, account_id, use_count
             FROM payee_memory
             WHERE household_id = ?
             ORDER BY use_count DESC, last_used_ms DESC
             LIMIT ?",
        )
        .bind(household_id)
        .bind(n as i64)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(name, account_id, count)| MemoryHint {
            payee_name: name,
            account_id,
            use_count: count as u32,
        })
        .collect()
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
        let path = dir.path().join("pm_test.db");
        let pool = create_encrypted_db(&path, "pw", &[0u8; 16]).await.unwrap();
        run_migrations(&pool).await.unwrap();

        let hid = new_ulid();
        sqlx::query(
            "INSERT INTO households (id, name, timezone, created_at) VALUES (?, 'H', 'UTC', 0)",
        )
        .bind(&hid)
        .execute(&pool)
        .await
        .unwrap();

        // Insert a placeholder account so FK succeeds.
        let aid = new_ulid();
        sqlx::query(
            "INSERT INTO accounts (id, household_id, name, type, normal_balance, created_at)
             VALUES (?, ?, 'Groceries', 'expense', 'debit', 0)",
        )
        .bind(&aid)
        .bind(&hid)
        .execute(&pool)
        .await
        .unwrap();

        (pool, hid)
    }

    async fn account_id(pool: &SqlitePool, hid: &str) -> String {
        sqlx::query_as::<_, (String,)>("SELECT id FROM accounts WHERE household_id = ? LIMIT 1")
            .bind(hid)
            .fetch_one(pool)
            .await
            .unwrap()
            .0
    }

    #[tokio::test]
    async fn lookup_returns_none_for_unknown_payee() {
        let (pool, hid) = setup().await;
        let mem = PayeeMemory::new(pool);
        assert!(mem.lookup(&hid, "Whole Foods").await.is_none());
    }

    #[tokio::test]
    async fn lookup_returns_mapping_after_record() {
        let (pool, hid) = setup().await;
        let aid = account_id(&pool, &hid).await;
        let mem = PayeeMemory::new(pool);

        mem.record(&hid, "Whole Foods", &aid).await.unwrap();
        let hint = mem.lookup(&hid, "Whole Foods").await.unwrap();
        assert_eq!(hint.account_id, aid);
        assert_eq!(hint.use_count, 1);
    }

    #[tokio::test]
    async fn record_increments_use_count() {
        let (pool, hid) = setup().await;
        let aid = account_id(&pool, &hid).await;
        let mem = PayeeMemory::new(pool);

        mem.record(&hid, "Netflix", &aid).await.unwrap();
        mem.record(&hid, "Netflix", &aid).await.unwrap();

        let hint = mem.lookup(&hid, "Netflix").await.unwrap();
        assert_eq!(hint.use_count, 2);
    }

    #[tokio::test]
    async fn lookup_is_case_insensitive() {
        let (pool, hid) = setup().await;
        let aid = account_id(&pool, &hid).await;
        let mem = PayeeMemory::new(pool);

        mem.record(&hid, "whole foods", &aid).await.unwrap();
        let hint = mem.lookup(&hid, "WHOLE FOODS").await.unwrap();
        assert_eq!(hint.account_id, aid);
    }

    #[tokio::test]
    async fn top_hints_returns_most_used() {
        let (pool, hid) = setup().await;
        let aid = account_id(&pool, &hid).await;
        let mem = PayeeMemory::new(pool);

        mem.record(&hid, "A", &aid).await.unwrap();
        mem.record(&hid, "B", &aid).await.unwrap();
        mem.record(&hid, "B", &aid).await.unwrap();
        mem.record(&hid, "B", &aid).await.unwrap();

        let hints = mem.top_hints(&hid, 1).await;
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].payee_name, "B");
        assert_eq!(hints[0].use_count, 3);
    }

    #[tokio::test]
    async fn top_hints_empty_for_new_household() {
        let (pool, hid) = setup().await;
        let mem = PayeeMemory::new(pool);
        assert!(mem.top_hints(&hid, 5).await.is_empty());
    }
}
