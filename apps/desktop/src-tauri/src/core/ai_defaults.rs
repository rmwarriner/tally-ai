//! Persistent AI defaults (#144).
//!
//! Stores per-household defaults that shape AI prompts and auto-confirm
//! behavior. Initial keys:
//!
//! - `default_payment_account`  — ULID of the account to default to
//! - `default_currency`         — currency code (Phase 1 always "USD")
//! - `confirm_threshold_cents`  — auto-confirm txns at or below this amount
//!
//! Values are stored as plain text; the AI orchestrator parses as needed at
//! the use site.

use std::collections::HashMap;

use sqlx::SqlitePool;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AiDefaultsError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Unknown key: {0}")]
    UnknownKey(String),
    #[error("Invalid value for {key}: {reason}")]
    InvalidValue { key: String, reason: String },
}

/// Keys we currently know how to validate. Unknown keys are rejected from
/// `set` to keep the schema honest — adding a new default key is a code
/// change, not a runtime free-for-all.
pub const KNOWN_KEYS: &[&str] = &[
    "default_payment_account",
    "default_currency",
    "confirm_threshold_cents",
];

/// Fetch every default for a household.
pub async fn get_all(
    pool: &SqlitePool,
    household_id: &str,
) -> Result<HashMap<String, String>, AiDefaultsError> {
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT key, value FROM ai_defaults WHERE household_id = ?")
            .bind(household_id)
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().collect())
}

/// Set (insert or update) one default value. Validates the key + value.
pub async fn set(
    pool: &SqlitePool,
    household_id: &str,
    key: &str,
    value: &str,
    now_ms: i64,
) -> Result<(), AiDefaultsError> {
    if !KNOWN_KEYS.contains(&key) {
        return Err(AiDefaultsError::UnknownKey(key.to_string()));
    }
    validate(key, value)?;

    sqlx::query(
        "INSERT INTO ai_defaults (household_id, key, value, updated_at) VALUES (?, ?, ?, ?)
         ON CONFLICT(household_id, key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
    )
    .bind(household_id)
    .bind(key)
    .bind(value)
    .bind(now_ms)
    .execute(pool)
    .await?;
    Ok(())
}

fn validate(key: &str, value: &str) -> Result<(), AiDefaultsError> {
    match key {
        "confirm_threshold_cents" => {
            if value.parse::<i64>().is_err() {
                return Err(AiDefaultsError::InvalidValue {
                    key: key.to_string(),
                    reason: "expected an integer number of cents".to_string(),
                });
            }
        }
        "default_currency" => {
            if value.len() != 3 || !value.chars().all(|c| c.is_ascii_uppercase()) {
                return Err(AiDefaultsError::InvalidValue {
                    key: key.to_string(),
                    reason: "expected a 3-letter uppercase currency code".to_string(),
                });
            }
        }
        // ULIDs are 26 chars Crockford Base32 — accept loosely.
        "default_payment_account" => {
            if value.trim().is_empty() {
                return Err(AiDefaultsError::InvalidValue {
                    key: key.to_string(),
                    reason: "expected a non-empty account ULID".to_string(),
                });
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{connection::create_encrypted_db, migrations::run_migrations};
    use crate::id::new_ulid;
    use tempfile::tempdir;

    async fn setup() -> (tempfile::TempDir, SqlitePool, String) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("t.db");
        let pool = create_encrypted_db(&path, "pp", &[0u8; 16]).await.unwrap();
        run_migrations(&pool).await.unwrap();
        let hh = new_ulid();
        sqlx::query(
            "INSERT INTO households (id, name, timezone, created_at) VALUES (?, 'H', 'UTC', 0)",
        )
        .bind(&hh)
        .execute(&pool)
        .await
        .unwrap();
        (dir, pool, hh)
    }

    #[tokio::test]
    async fn set_then_get_roundtrips() {
        let (_d, pool, hh) = setup().await;
        set(&pool, &hh, "default_currency", "USD", 1000).await.unwrap();
        let all = get_all(&pool, &hh).await.unwrap();
        assert_eq!(all.get("default_currency"), Some(&"USD".to_string()));
    }

    #[tokio::test]
    async fn set_upserts_existing_key() {
        let (_d, pool, hh) = setup().await;
        set(&pool, &hh, "default_currency", "USD", 1000).await.unwrap();
        set(&pool, &hh, "default_currency", "EUR", 2000).await.unwrap();
        let all = get_all(&pool, &hh).await.unwrap();
        assert_eq!(all.get("default_currency"), Some(&"EUR".to_string()));
        assert_eq!(all.len(), 1);
    }

    #[tokio::test]
    async fn get_all_returns_empty_for_new_household() {
        let (_d, pool, hh) = setup().await;
        let all = get_all(&pool, &hh).await.unwrap();
        assert!(all.is_empty());
    }

    #[tokio::test]
    async fn unknown_keys_are_rejected() {
        let (_d, pool, hh) = setup().await;
        let err = set(&pool, &hh, "random_thing", "x", 1000).await.unwrap_err();
        assert!(matches!(err, AiDefaultsError::UnknownKey(_)));
    }

    #[tokio::test]
    async fn confirm_threshold_must_be_an_integer() {
        let (_d, pool, hh) = setup().await;
        let err = set(&pool, &hh, "confirm_threshold_cents", "five hundred", 1000)
            .await
            .unwrap_err();
        assert!(matches!(err, AiDefaultsError::InvalidValue { .. }));
    }

    #[tokio::test]
    async fn currency_must_be_three_letter_uppercase() {
        let (_d, pool, hh) = setup().await;
        assert!(set(&pool, &hh, "default_currency", "usd", 1000)
            .await
            .is_err());
        assert!(set(&pool, &hh, "default_currency", "USDX", 1000)
            .await
            .is_err());
        assert!(set(&pool, &hh, "default_currency", "USD", 1000).await.is_ok());
    }

    #[tokio::test]
    async fn payment_account_rejects_empty_value() {
        let (_d, pool, hh) = setup().await;
        let err = set(&pool, &hh, "default_payment_account", "  ", 1000)
            .await
            .unwrap_err();
        assert!(matches!(err, AiDefaultsError::InvalidValue { .. }));
    }
}
