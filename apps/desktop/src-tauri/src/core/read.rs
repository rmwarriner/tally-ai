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

#[derive(sqlx::FromRow)]
struct EnvelopeRow {
    envelope_id: String,
    name: String,
    allocated: i64,
    spent: i64,
}

/// Returns every envelope in the household paired with its allocated/spent
/// for the period containing `as_of_ms`. LEFT JOIN: envelopes with no matching
/// period appear with zeros rather than being dropped.
pub async fn current_envelope_periods(
    pool: &SqlitePool,
    household_id: &str,
    as_of_ms: i64,
) -> Result<Vec<EnvelopeStatus>, sqlx::Error> {
    let rows = sqlx::query_as::<_, EnvelopeRow>(
        r#"
        SELECT
            e.id   AS envelope_id,
            e.name AS name,
            COALESCE(ep.allocated, 0) AS allocated,
            COALESCE(ep.spent, 0)     AS spent
        FROM envelopes e
        LEFT JOIN envelope_periods ep
          ON ep.envelope_id = e.id
         AND ep.period_start <= ?
         AND ep.period_end   >= ?
        WHERE e.household_id = ?
        ORDER BY e.name
        "#,
    )
    .bind(as_of_ms)
    .bind(as_of_ms)
    .bind(household_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| EnvelopeStatus {
            envelope_id: r.envelope_id,
            name: r.name,
            allocated_cents: r.allocated,
            spent_cents: r.spent,
        })
        .collect())
}

#[derive(sqlx::FromRow)]
struct ComingUpRow {
    id: String,
    txn_date: i64,
    status: String,
    memo: Option<String>,
    amount_cents: i64,
}

/// Returns the union of pending proposals and future-dated posted
/// transactions, ordered by txn_date ascending, limited to `limit` rows.
/// `amount_cents` is the sum of debits on expense accounts; if none,
/// falls back to the sum of debits on asset accounts; else 0.
pub async fn coming_up_transactions(
    pool: &SqlitePool,
    household_id: &str,
    as_of_ms: i64,
    limit: i64,
) -> Result<Vec<ComingUpTxn>, sqlx::Error> {
    let rows = sqlx::query_as::<_, ComingUpRow>(
        r#"
        SELECT
            t.id,
            t.txn_date,
            t.status,
            t.memo,
            COALESCE(
              (SELECT SUM(jl.amount) FROM journal_lines jl
                 JOIN accounts a ON a.id = jl.account_id
               WHERE jl.transaction_id = t.id
                 AND jl.side = 'debit'
                 AND a.type = 'expense'),
              (SELECT SUM(jl.amount) FROM journal_lines jl
                 JOIN accounts a ON a.id = jl.account_id
               WHERE jl.transaction_id = t.id
                 AND jl.side = 'debit'
                 AND a.type = 'asset'),
              0
            ) AS amount_cents
        FROM transactions t
        WHERE t.household_id = ?
          AND (t.status = 'pending' OR (t.status = 'posted' AND t.txn_date > ?))
        ORDER BY t.txn_date ASC
        LIMIT ?
        "#,
    )
    .bind(household_id)
    .bind(as_of_ms)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ComingUpTxn {
            id: r.id,
            txn_date: r.txn_date,
            status: r.status,
            payee: r.memo.clone(),
            memo: r.memo,
            amount_cents: r.amount_cents,
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

    async fn insert_envelope(pool: &SqlitePool, hid: &str, acct_id: &str, name: &str) -> String {
        let id = new_ulid();
        sqlx::query(
            "INSERT INTO envelopes (id, household_id, account_id, name, created_at)
             VALUES (?, ?, ?, ?, 0)",
        )
        .bind(&id)
        .bind(hid)
        .bind(acct_id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
        id
    }

    async fn insert_period(
        pool: &SqlitePool,
        envelope_id: &str,
        start: i64,
        end: i64,
        allocated: i64,
        spent: i64,
    ) {
        sqlx::query(
            "INSERT INTO envelope_periods
               (id, envelope_id, period_start, period_end, allocated, spent, created_at)
             VALUES (?, ?, ?, ?, ?, ?, 0)",
        )
        .bind(new_ulid())
        .bind(envelope_id)
        .bind(start)
        .bind(end)
        .bind(allocated)
        .bind(spent)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn envelopes_returns_zeros_when_no_current_period() {
        let (pool, hid) = setup().await;
        let groceries = insert_account(&pool, &hid, "Groceries", "expense", "debit", false).await;
        insert_envelope(&pool, &hid, &groceries, "Groceries").await;

        let envelopes = current_envelope_periods(&pool, &hid, 1_000).await.unwrap();
        assert_eq!(envelopes.len(), 1);
        assert_eq!(envelopes[0].name, "Groceries");
        assert_eq!(envelopes[0].allocated_cents, 0);
        assert_eq!(envelopes[0].spent_cents, 0);
    }

    #[tokio::test]
    async fn envelopes_returns_matching_period() {
        let (pool, hid) = setup().await;
        let acct = insert_account(&pool, &hid, "Groceries", "expense", "debit", false).await;
        let env = insert_envelope(&pool, &hid, &acct, "Groceries").await;
        insert_period(&pool, &env, 0, 9_999_999_999_999, 50_000, 20_000).await;

        let envelopes = current_envelope_periods(&pool, &hid, 1_000).await.unwrap();
        assert_eq!(envelopes[0].allocated_cents, 50_000);
        assert_eq!(envelopes[0].spent_cents, 20_000);
    }

    #[tokio::test]
    async fn envelopes_ignores_periods_outside_as_of() {
        let (pool, hid) = setup().await;
        let acct = insert_account(&pool, &hid, "Gas", "expense", "debit", false).await;
        let env = insert_envelope(&pool, &hid, &acct, "Gas").await;
        insert_period(&pool, &env, 0, 100, 10_000, 500).await;

        let envelopes = current_envelope_periods(&pool, &hid, 1_000).await.unwrap();
        assert_eq!(envelopes[0].allocated_cents, 0);
        assert_eq!(envelopes[0].spent_cents, 0);
    }

    #[tokio::test]
    async fn envelopes_isolate_households() {
        let (pool, hid_a) = setup().await;
        let hid_b = new_ulid();
        sqlx::query(
            "INSERT INTO households (id, name, timezone, created_at) VALUES (?, 'B', 'UTC', 0)",
        )
        .bind(&hid_b)
        .execute(&pool)
        .await
        .unwrap();
        let acct_a = insert_account(&pool, &hid_a, "A-Groc", "expense", "debit", false).await;
        let acct_b = insert_account(&pool, &hid_b, "B-Groc", "expense", "debit", false).await;
        insert_envelope(&pool, &hid_a, &acct_a, "A-Groc").await;
        insert_envelope(&pool, &hid_b, &acct_b, "B-Groc").await;

        let a = current_envelope_periods(&pool, &hid_a, 1_000).await.unwrap();
        assert!(a.iter().all(|e| e.name != "B-Groc"));
        assert!(a.iter().any(|e| e.name == "A-Groc"));
    }

    #[tokio::test]
    async fn coming_up_includes_pending_with_expense_amount() {
        let (pool, hid) = setup().await;
        let asset = insert_account(&pool, &hid, "Checking", "asset", "debit", false).await;
        let expense = insert_account(&pool, &hid, "Groceries", "expense", "debit", false).await;

        let tid = insert_txn(&pool, &hid, "pending", 1_000).await;
        sqlx::query(
            "UPDATE transactions SET memo = 'Trader Joes' WHERE id = ?",
        )
        .bind(&tid)
        .execute(&pool)
        .await
        .unwrap();
        insert_line(&pool, &tid, &expense, 4_200, "debit").await;
        insert_line(&pool, &tid, &asset, 4_200, "credit").await;

        let items = coming_up_transactions(&pool, &hid, 5_000, 50).await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].status, "pending");
        assert_eq!(items[0].payee.as_deref(), Some("Trader Joes"));
        assert_eq!(items[0].memo.as_deref(), Some("Trader Joes"));
        assert_eq!(items[0].amount_cents, 4_200);
    }

    #[tokio::test]
    async fn coming_up_includes_future_posted() {
        let (pool, hid) = setup().await;
        let asset = insert_account(&pool, &hid, "Checking", "asset", "debit", false).await;
        let equity = insert_account(&pool, &hid, "Equity", "equity", "credit", false).await;

        let tid = insert_txn(&pool, &hid, "posted", 10_000).await;
        insert_line(&pool, &tid, &asset, 1_000, "debit").await;
        insert_line(&pool, &tid, &equity, 1_000, "credit").await;

        let items = coming_up_transactions(&pool, &hid, 1_000, 50).await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].status, "posted");
        // No expense line; amount falls back to sum of asset debits.
        assert_eq!(items[0].amount_cents, 1_000);
    }

    #[tokio::test]
    async fn coming_up_excludes_past_posted() {
        let (pool, hid) = setup().await;
        let asset = insert_account(&pool, &hid, "Checking", "asset", "debit", false).await;
        let equity = insert_account(&pool, &hid, "Equity", "equity", "credit", false).await;

        let tid = insert_txn(&pool, &hid, "posted", 0).await;
        insert_line(&pool, &tid, &asset, 1_000, "debit").await;
        insert_line(&pool, &tid, &equity, 1_000, "credit").await;

        let items = coming_up_transactions(&pool, &hid, 5_000, 50).await.unwrap();
        assert_eq!(items.len(), 0);
    }

    #[tokio::test]
    async fn coming_up_respects_limit_and_orders_by_date() {
        let (pool, hid) = setup().await;
        let asset = insert_account(&pool, &hid, "Checking", "asset", "debit", false).await;
        let equity = insert_account(&pool, &hid, "Equity", "equity", "credit", false).await;

        for d in [3_000_i64, 1_000, 2_000] {
            let tid = insert_txn(&pool, &hid, "pending", d).await;
            insert_line(&pool, &tid, &asset, 100, "debit").await;
            insert_line(&pool, &tid, &equity, 100, "credit").await;
        }

        let items = coming_up_transactions(&pool, &hid, 0, 2).await.unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].txn_date, 1_000);
        assert_eq!(items[1].txn_date, 2_000);
    }
}
