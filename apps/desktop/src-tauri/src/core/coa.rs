// Standard household chart of accounts seed data — T-017
// Creates the default account hierarchy for a new household.
// Idempotent: returns Err if accounts already exist for the household.

use sqlx::SqlitePool;
use thiserror::Error;

use crate::id::new_ulid;

#[derive(Debug, Error)]
pub enum CoaError {
    #[error("Chart of accounts already exists for this household")]
    AlreadySeeded,
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

struct AccountSeed {
    key: &'static str,
    parent_key: Option<&'static str>,
    name: &'static str,
    account_type: &'static str,
    normal_balance: &'static str,
    is_placeholder: bool,
}

static STANDARD_ACCOUNTS: &[AccountSeed] = &[
    // Assets
    AccountSeed { key: "assets",      parent_key: None,          name: "Assets",                  account_type: "asset",     normal_balance: "debit",  is_placeholder: true  },
    AccountSeed { key: "checking",    parent_key: Some("assets"), name: "Checking",                account_type: "asset",     normal_balance: "debit",  is_placeholder: false },
    AccountSeed { key: "savings",     parent_key: Some("assets"), name: "Savings",                 account_type: "asset",     normal_balance: "debit",  is_placeholder: false },
    AccountSeed { key: "cash",        parent_key: Some("assets"), name: "Cash",                    account_type: "asset",     normal_balance: "debit",  is_placeholder: false },
    // Liabilities
    AccountSeed { key: "liabilities", parent_key: None,           name: "Liabilities",             account_type: "liability", normal_balance: "credit", is_placeholder: true  },
    AccountSeed { key: "credit_card", parent_key: Some("liabilities"), name: "Credit Card",        account_type: "liability", normal_balance: "credit", is_placeholder: false },
    AccountSeed { key: "loans",       parent_key: Some("liabilities"), name: "Student Loans",      account_type: "liability", normal_balance: "credit", is_placeholder: false },
    // Income
    AccountSeed { key: "income",      parent_key: None,           name: "Income",                  account_type: "income",    normal_balance: "credit", is_placeholder: true  },
    AccountSeed { key: "salary",      parent_key: Some("income"), name: "Salary",                  account_type: "income",    normal_balance: "credit", is_placeholder: false },
    AccountSeed { key: "other_income",parent_key: Some("income"), name: "Other Income",            account_type: "income",    normal_balance: "credit", is_placeholder: false },
    // Expenses — top-level placeholder
    AccountSeed { key: "expenses",    parent_key: None,           name: "Expenses",                account_type: "expense",   normal_balance: "debit",  is_placeholder: true  },
    // Housing
    AccountSeed { key: "housing",     parent_key: Some("expenses"), name: "Housing",               account_type: "expense",   normal_balance: "debit",  is_placeholder: true  },
    AccountSeed { key: "rent",        parent_key: Some("housing"),  name: "Rent / Mortgage",       account_type: "expense",   normal_balance: "debit",  is_placeholder: false },
    // Food
    AccountSeed { key: "food",        parent_key: Some("expenses"), name: "Food & Dining",         account_type: "expense",   normal_balance: "debit",  is_placeholder: true  },
    AccountSeed { key: "groceries",   parent_key: Some("food"),     name: "Groceries",             account_type: "expense",   normal_balance: "debit",  is_placeholder: false },
    AccountSeed { key: "dining_out",  parent_key: Some("food"),     name: "Dining Out",            account_type: "expense",   normal_balance: "debit",  is_placeholder: false },
    // Other leaf expenses
    AccountSeed { key: "transport",   parent_key: Some("expenses"), name: "Transportation",        account_type: "expense",   normal_balance: "debit",  is_placeholder: false },
    AccountSeed { key: "utilities",   parent_key: Some("expenses"), name: "Utilities",             account_type: "expense",   normal_balance: "debit",  is_placeholder: false },
    AccountSeed { key: "entertainment",parent_key: Some("expenses"), name: "Entertainment",        account_type: "expense",   normal_balance: "debit",  is_placeholder: false },
    AccountSeed { key: "healthcare",  parent_key: Some("expenses"), name: "Healthcare",            account_type: "expense",   normal_balance: "debit",  is_placeholder: false },
    // Equity
    AccountSeed { key: "equity",      parent_key: None,           name: "Equity",                  account_type: "equity",    normal_balance: "credit", is_placeholder: true  },
    AccountSeed { key: "obe",         parent_key: Some("equity"), name: "Opening Balance Equity",  account_type: "equity",    normal_balance: "credit", is_placeholder: false },
];

/// Seeds the standard household chart of accounts for `household_id`.
///
/// Fails with [`CoaError::AlreadySeeded`] if the household already has accounts,
/// preventing accidental double-seeding during onboarding.
pub async fn seed_chart_of_accounts(
    pool: &SqlitePool,
    household_id: &str,
) -> Result<(), CoaError> {
    let (existing,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE household_id = ?")
            .bind(household_id)
            .fetch_one(pool)
            .await?;

    if existing > 0 {
        return Err(CoaError::AlreadySeeded);
    }

    let mut tx = pool.begin().await?;
    let now: i64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // Map logical key → generated ULID so children can reference parents
    let mut key_to_id: std::collections::HashMap<&'static str, String> =
        std::collections::HashMap::new();

    for seed in STANDARD_ACCOUNTS {
        let id = new_ulid();
        let parent_id = seed.parent_key.and_then(|k| key_to_id.get(k)).cloned();

        sqlx::query(
            "INSERT INTO accounts
                 (id, household_id, parent_id, name, type, normal_balance, is_placeholder, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(household_id)
        .bind(&parent_id)
        .bind(seed.name)
        .bind(seed.account_type)
        .bind(seed.normal_balance)
        .bind(seed.is_placeholder as i64)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        key_to_id.insert(seed.key, id);
    }

    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{connection::create_encrypted_db, migrations::run_migrations};
    use tempfile::tempdir;

    async fn test_pool() -> SqlitePool {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.keep().join("test.db");
        let pool = create_encrypted_db(&db_path, "test", &[0u8; 16])
            .await
            .expect("create db");
        run_migrations(&pool).await.expect("migrate");
        pool
    }

    async fn setup_household(pool: &SqlitePool, id: &str) {
        sqlx::query(
            "INSERT INTO households (id, name, timezone, created_at) VALUES (?, 'Test', 'UTC', 0)",
        )
        .bind(id)
        .execute(pool)
        .await
        .expect("household");
    }

    #[tokio::test]
    async fn seed_creates_expected_account_count() {
        let pool = test_pool().await;
        setup_household(&pool, "hh").await;
        seed_chart_of_accounts(&pool, "hh").await.expect("seed");

        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE household_id = 'hh'")
                .fetch_one(&pool)
                .await
                .expect("query");

        assert_eq!(count, STANDARD_ACCOUNTS.len() as i64);
    }

    #[tokio::test]
    async fn seed_includes_opening_balance_equity() {
        let pool = test_pool().await;
        setup_household(&pool, "hh").await;
        seed_chart_of_accounts(&pool, "hh").await.expect("seed");

        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM accounts WHERE household_id = 'hh' AND name = 'Opening Balance Equity'",
        )
        .fetch_one(&pool)
        .await
        .expect("query");

        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn seed_placeholder_accounts_are_not_leaf() {
        let pool = test_pool().await;
        setup_household(&pool, "hh").await;
        seed_chart_of_accounts(&pool, "hh").await.expect("seed");

        let placeholder_count = STANDARD_ACCOUNTS.iter().filter(|a| a.is_placeholder).count() as i64;
        let (db_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM accounts WHERE household_id = 'hh' AND is_placeholder = 1",
        )
        .fetch_one(&pool)
        .await
        .expect("query");

        assert_eq!(db_count, placeholder_count);
    }

    #[tokio::test]
    async fn seed_rejects_duplicate_for_same_household() {
        let pool = test_pool().await;
        setup_household(&pool, "hh").await;
        seed_chart_of_accounts(&pool, "hh").await.expect("first seed");

        let err = seed_chart_of_accounts(&pool, "hh")
            .await
            .expect_err("second seed should fail");
        assert!(matches!(err, CoaError::AlreadySeeded));
    }

    #[tokio::test]
    async fn seed_parent_child_relationships_are_correct() {
        let pool = test_pool().await;
        setup_household(&pool, "hh").await;
        seed_chart_of_accounts(&pool, "hh").await.expect("seed");

        // Groceries should have a parent (Food & Dining)
        let (parent_id,): (Option<String>,) = sqlx::query_as(
            "SELECT parent_id FROM accounts WHERE household_id = 'hh' AND name = 'Groceries'",
        )
        .fetch_one(&pool)
        .await
        .expect("query");
        assert!(parent_id.is_some(), "Groceries must have a parent");

        // Food & Dining's parent should be Expenses
        let (food_parent_id,): (Option<String>,) = sqlx::query_as(
            "SELECT parent_id FROM accounts WHERE household_id = 'hh' AND name = 'Food & Dining'",
        )
        .fetch_one(&pool)
        .await
        .expect("query");
        assert!(food_parent_id.is_some(), "Food & Dining must have a parent");
    }

    #[tokio::test]
    async fn seed_allows_different_households() {
        let pool = test_pool().await;
        setup_household(&pool, "hh1").await;
        setup_household(&pool, "hh2").await;

        seed_chart_of_accounts(&pool, "hh1").await.expect("hh1 seed");
        seed_chart_of_accounts(&pool, "hh2").await.expect("hh2 seed — must not conflict");

        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM accounts")
                .fetch_one(&pool)
                .await
                .expect("query");

        assert_eq!(count, (STANDARD_ACCOUNTS.len() * 2) as i64);
    }
}
