// Read-only queries backing the health sidebar — T-048.
// These queries are the single source of truth for balance math; the AI
// snapshot layer delegates here so there is only one formula.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBalance {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub account_type: String,
    /// Signed balance in cents: positive = normal balance direction.
    pub balance_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvelopeStatus {
    pub envelope_id: String,
    pub name: String,
    pub allocated_cents: i64,
    pub spent_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComingUpTxn {
    pub id: String,
    pub txn_date: i64,
    pub status: String,
    pub payee: Option<String>,
    pub memo: Option<String>,
    pub amount_cents: i64,
}

#[derive(sqlx::FromRow)]
struct BalanceRow {
    id: String,
    name: String,
    account_type: String,
    normal_balance: String,
    debit_total: i64,
    credit_total: i64,
}

/// Returns every non-placeholder account in the household with its signed
/// balance in cents. Pending transactions are excluded from the sums.
pub async fn account_balances(
    pool: &SqlitePool,
    household_id: &str,
) -> Result<Vec<AccountBalance>, sqlx::Error> {
    let rows = sqlx::query_as::<_, BalanceRow>(
        r#"
        SELECT
            a.id,
            a.name,
            a.type       AS account_type,
            a.normal_balance,
            COALESCE(SUM(CASE WHEN jl.side = 'debit'  AND t.status = 'posted' THEN jl.amount ELSE 0 END), 0) AS debit_total,
            COALESCE(SUM(CASE WHEN jl.side = 'credit' AND t.status = 'posted' THEN jl.amount ELSE 0 END), 0) AS credit_total
        FROM accounts a
        LEFT JOIN journal_lines jl ON jl.account_id = a.id
        LEFT JOIN transactions  t  ON t.id = jl.transaction_id
        WHERE a.household_id = ?
          AND a.is_placeholder = 0
        GROUP BY a.id
        ORDER BY a.type, a.name
        "#,
    )
    .bind(household_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let balance_cents = if r.normal_balance == "debit" {
                r.debit_total - r.credit_total
            } else {
                r.credit_total - r.debit_total
            };
            AccountBalance {
                id: r.id,
                name: r.name,
                account_type: r.account_type,
                balance_cents,
            }
        })
        .collect())
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
        let path = dir.path().join("read_test.db");
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

    async fn insert_account(
        pool: &SqlitePool,
        hid: &str,
        name: &str,
        acct_type: &str,
        normal_balance: &str,
        is_placeholder: bool,
    ) -> String {
        let id = new_ulid();
        sqlx::query(
            "INSERT INTO accounts (id, household_id, name, type, normal_balance, is_placeholder, created_at)
             VALUES (?, ?, ?, ?, ?, ?, 0)",
        )
        .bind(&id)
        .bind(hid)
        .bind(name)
        .bind(acct_type)
        .bind(normal_balance)
        .bind(is_placeholder)
        .execute(pool)
        .await
        .unwrap();
        id
    }

    async fn insert_txn(pool: &SqlitePool, hid: &str, status: &str, date_ms: i64) -> String {
        let tid = new_ulid();
        sqlx::query(
            "INSERT INTO transactions
               (id, household_id, txn_date, entry_date, status, source, created_at)
             VALUES (?, ?, ?, 0, ?, 'ai', 0)",
        )
        .bind(&tid)
        .bind(hid)
        .bind(date_ms)
        .bind(status)
        .execute(pool)
        .await
        .unwrap();
        tid
    }

    async fn insert_line(pool: &SqlitePool, tid: &str, account_id: &str, amount: i64, side: &str) {
        sqlx::query(
            "INSERT INTO journal_lines (id, transaction_id, account_id, amount, side, created_at)
             VALUES (?, ?, ?, ?, ?, 0)",
        )
        .bind(new_ulid())
        .bind(tid)
        .bind(account_id)
        .bind(amount)
        .bind(side)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn account_balances_sums_posted_debits_for_asset() {
        let (pool, hid) = setup().await;
        let checking = insert_account(&pool, &hid, "Checking", "asset", "debit", false).await;
        let equity = insert_account(&pool, &hid, "Equity", "equity", "credit", false).await;

        let tid = insert_txn(&pool, &hid, "posted", 0).await;
        insert_line(&pool, &tid, &checking, 12_345, "debit").await;
        insert_line(&pool, &tid, &equity, 12_345, "credit").await;

        let balances = account_balances(&pool, &hid).await.unwrap();
        let bal = balances.iter().find(|b| b.name == "Checking").unwrap();
        assert_eq!(bal.balance_cents, 12_345);
    }

    #[tokio::test]
    async fn account_balances_excludes_pending() {
        let (pool, hid) = setup().await;
        let checking = insert_account(&pool, &hid, "Checking", "asset", "debit", false).await;
        let equity = insert_account(&pool, &hid, "Equity", "equity", "credit", false).await;

        let tid = insert_txn(&pool, &hid, "pending", 0).await;
        insert_line(&pool, &tid, &checking, 50_000, "debit").await;
        insert_line(&pool, &tid, &equity, 50_000, "credit").await;

        let balances = account_balances(&pool, &hid).await.unwrap();
        let bal = balances.iter().find(|b| b.name == "Checking").unwrap();
        assert_eq!(bal.balance_cents, 0);
    }

    #[tokio::test]
    async fn account_balances_excludes_placeholders() {
        let (pool, hid) = setup().await;
        insert_account(&pool, &hid, "Assets", "asset", "debit", true).await;
        insert_account(&pool, &hid, "Checking", "asset", "debit", false).await;

        let balances = account_balances(&pool, &hid).await.unwrap();
        assert!(balances.iter().all(|b| b.name != "Assets"));
        assert!(balances.iter().any(|b| b.name == "Checking"));
    }

    #[tokio::test]
    async fn account_balances_isolates_households() {
        let (pool, hid_a) = setup().await;
        let hid_b = new_ulid();
        sqlx::query(
            "INSERT INTO households (id, name, timezone, created_at) VALUES (?, 'B', 'UTC', 0)",
        )
        .bind(&hid_b)
        .execute(&pool)
        .await
        .unwrap();

        insert_account(&pool, &hid_a, "A-Checking", "asset", "debit", false).await;
        insert_account(&pool, &hid_b, "B-Checking", "asset", "debit", false).await;

        let a_bals = account_balances(&pool, &hid_a).await.unwrap();
        assert!(a_bals.iter().all(|b| b.name != "B-Checking"));
    }

    #[tokio::test]
    async fn account_balances_credit_normal_liability_is_positive_when_credited() {
        let (pool, hid) = setup().await;
        let credit_card = insert_account(&pool, &hid, "Visa", "liability", "credit", false).await;
        let expense = insert_account(&pool, &hid, "Groceries", "expense", "debit", false).await;

        let tid = insert_txn(&pool, &hid, "posted", 0).await;
        insert_line(&pool, &tid, &expense, 2_500, "debit").await;
        insert_line(&pool, &tid, &credit_card, 2_500, "credit").await;

        let balances = account_balances(&pool, &hid).await.unwrap();
        let visa = balances.iter().find(|b| b.name == "Visa").unwrap();
        assert_eq!(visa.balance_cents, 2_500);
    }
}
