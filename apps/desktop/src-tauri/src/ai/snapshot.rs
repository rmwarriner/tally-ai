// Financial snapshot builder — T-023
// Queries posted balances and current-period envelope health.
// Scheduled transactions are Phase 2; see TODO(phase2) below.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBalance {
    pub account_id: String,
    pub account_name: String,
    pub account_type: String,
    /// Signed balance in cents: positive = normal balance direction.
    pub balance_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvelopeHealth {
    pub envelope_id: String,
    pub envelope_name: String,
    pub allocated_cents: i64,
    pub spent_cents: i64,
    pub remaining_cents: i64,
    /// UTC ms of period end.
    pub period_end_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinancialSnapshot {
    pub household_id: String,
    /// Unix ms at which the snapshot was taken.
    pub as_of_ms: i64,
    pub balances: Vec<AccountBalance>,
    pub envelopes: Vec<EnvelopeHealth>,
    // TODO(phase2): scheduled: Vec<ScheduledTransaction>
}

impl FinancialSnapshot {
    /// Formats the snapshot as a concise prompt string for the SNAPSHOT layer.
    pub fn to_prompt_text(&self) -> String {
        let mut out = String::from("=== Financial Snapshot ===\n");

        if self.balances.is_empty() {
            out.push_str("Accounts: none\n");
        } else {
            out.push_str("Accounts\n");
            for b in &self.balances {
                if b.balance_cents != 0 {
                    out.push_str(&format!(
                        "  {} ({}): {}\n",
                        b.account_name,
                        b.account_type,
                        format_dollars(b.balance_cents)
                    ));
                }
            }
        }

        if !self.envelopes.is_empty() {
            out.push_str("\nEnvelopes (current period)\n");
            for e in &self.envelopes {
                let pct = if e.allocated_cents > 0 {
                    e.spent_cents * 100 / e.allocated_cents
                } else {
                    0
                };
                out.push_str(&format!(
                    "  {}: {}/{} ({}% used, {} remaining)\n",
                    e.envelope_name,
                    format_dollars(e.spent_cents),
                    format_dollars(e.allocated_cents),
                    pct,
                    format_dollars(e.remaining_cents)
                ));
            }
        }

        out
    }
}

pub async fn build_snapshot(
    pool: &SqlitePool,
    household_id: &str,
    as_of_ms: i64,
) -> Result<FinancialSnapshot, sqlx::Error> {
    let balances = query_balances(pool, household_id).await?;
    let envelopes = query_envelopes(pool, household_id, as_of_ms).await?;
    Ok(FinancialSnapshot { household_id: household_id.to_string(), as_of_ms, balances, envelopes })
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

async fn query_balances(
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
                account_id: r.id,
                account_name: r.name,
                account_type: r.account_type,
                balance_cents,
            }
        })
        .collect())
}

#[derive(sqlx::FromRow)]
struct EnvelopeRow {
    envelope_id: String,
    envelope_name: String,
    allocated: i64,
    spent: i64,
    period_end: i64,
}

async fn query_envelopes(
    pool: &SqlitePool,
    household_id: &str,
    as_of_ms: i64,
) -> Result<Vec<EnvelopeHealth>, sqlx::Error> {
    let rows = sqlx::query_as::<_, EnvelopeRow>(
        r#"
        SELECT
            e.id    AS envelope_id,
            e.name  AS envelope_name,
            ep.allocated,
            ep.spent,
            ep.period_end
        FROM envelopes e
        JOIN envelope_periods ep ON ep.envelope_id = e.id
        WHERE e.household_id = ?
          AND ep.period_start <= ?
          AND ep.period_end   >= ?
        ORDER BY e.name
        "#,
    )
    .bind(household_id)
    .bind(as_of_ms)
    .bind(as_of_ms)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| EnvelopeHealth {
            envelope_id: r.envelope_id,
            envelope_name: r.envelope_name,
            allocated_cents: r.allocated,
            spent_cents: r.spent,
            remaining_cents: r.allocated - r.spent,
            period_end_ms: r.period_end,
        })
        .collect())
}

fn format_dollars(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.unsigned_abs();
    format!("{}${}.{:02}", sign, abs / 100, abs % 100)
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
        let path = dir.path().join("snap_test.db");
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
    ) -> String {
        let id = new_ulid();
        sqlx::query(
            "INSERT INTO accounts (id, household_id, name, type, normal_balance, created_at)
             VALUES (?, ?, ?, ?, ?, 0)",
        )
        .bind(&id)
        .bind(hid)
        .bind(name)
        .bind(acct_type)
        .bind(normal_balance)
        .execute(pool)
        .await
        .unwrap();
        id
    }

    async fn insert_posted_txn(pool: &SqlitePool, hid: &str, date_ms: i64) -> String {
        let tid = new_ulid();
        sqlx::query(
            "INSERT INTO transactions
               (id, household_id, txn_date, entry_date, status, source, created_at)
             VALUES (?, ?, ?, 0, 'posted', 'ai', 0)",
        )
        .bind(&tid)
        .bind(hid)
        .bind(date_ms)
        .execute(pool)
        .await
        .unwrap();
        tid
    }

    async fn insert_line(
        pool: &SqlitePool,
        tid: &str,
        account_id: &str,
        amount: i64,
        side: &str,
    ) {
        let lid = new_ulid();
        sqlx::query(
            "INSERT INTO journal_lines (id, transaction_id, account_id, amount, side, created_at)
             VALUES (?, ?, ?, ?, ?, 0)",
        )
        .bind(lid)
        .bind(tid)
        .bind(account_id)
        .bind(amount)
        .bind(side)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn snapshot_includes_asset_balance() {
        let (pool, hid) = setup().await;
        let checking = insert_account(&pool, &hid, "Checking", "asset", "debit").await;
        let equity = insert_account(&pool, &hid, "Equity", "equity", "credit").await;
        let tid = insert_posted_txn(&pool, &hid, 0).await;
        insert_line(&pool, &tid, &checking, 10_000, "debit").await;
        insert_line(&pool, &tid, &equity, 10_000, "credit").await;

        let snap = build_snapshot(&pool, &hid, 0).await.unwrap();
        let checking_bal = snap.balances.iter().find(|b| b.account_name == "Checking").unwrap();
        assert_eq!(checking_bal.balance_cents, 10_000);
    }

    #[tokio::test]
    async fn snapshot_excludes_pending_transactions() {
        let (pool, hid) = setup().await;
        let checking = insert_account(&pool, &hid, "Checking", "asset", "debit").await;
        let equity = insert_account(&pool, &hid, "Equity", "equity", "credit").await;

        let tid = new_ulid();
        sqlx::query(
            "INSERT INTO transactions
               (id, household_id, txn_date, entry_date, status, source, created_at)
             VALUES (?, ?, 0, 0, 'pending', 'ai', 0)",
        )
        .bind(&tid)
        .bind(&hid)
        .execute(&pool)
        .await
        .unwrap();
        insert_line(&pool, &tid, &checking, 50_000, "debit").await;
        insert_line(&pool, &tid, &equity, 50_000, "credit").await;

        let snap = build_snapshot(&pool, &hid, 0).await.unwrap();
        let bal = snap.balances.iter().find(|b| b.account_name == "Checking").unwrap();
        assert_eq!(bal.balance_cents, 0);
    }

    #[tokio::test]
    async fn snapshot_includes_envelope_health() {
        let (pool, hid) = setup().await;
        let expense_acc = insert_account(&pool, &hid, "Groceries", "expense", "debit").await;

        let eid = new_ulid();
        sqlx::query(
            "INSERT INTO envelopes (id, household_id, account_id, name, created_at)
             VALUES (?, ?, ?, 'Groceries', 0)",
        )
        .bind(&eid)
        .bind(&hid)
        .bind(&expense_acc)
        .execute(&pool)
        .await
        .unwrap();

        let epid = new_ulid();
        sqlx::query(
            "INSERT INTO envelope_periods
               (id, envelope_id, period_start, period_end, allocated, spent, created_at)
             VALUES (?, ?, 0, 9999999999999, 50000, 20000, 0)",
        )
        .bind(epid)
        .bind(&eid)
        .execute(&pool)
        .await
        .unwrap();

        let snap = build_snapshot(&pool, &hid, 1_000).await.unwrap();
        assert_eq!(snap.envelopes.len(), 1);
        assert_eq!(snap.envelopes[0].allocated_cents, 50000);
        assert_eq!(snap.envelopes[0].spent_cents, 20000);
        assert_eq!(snap.envelopes[0].remaining_cents, 30000);
    }

    #[test]
    fn prompt_text_formats_balance() {
        let snap = FinancialSnapshot {
            household_id: "hid".to_string(),
            as_of_ms: 0,
            balances: vec![AccountBalance {
                account_id: "a1".to_string(),
                account_name: "Checking".to_string(),
                account_type: "asset".to_string(),
                balance_cents: 123_456,
            }],
            envelopes: vec![],
        };
        let text = snap.to_prompt_text();
        assert!(text.contains("Checking"));
        assert!(text.contains("$1234.56"));
    }

    #[test]
    fn prompt_text_skips_zero_balance_accounts() {
        let snap = FinancialSnapshot {
            household_id: "hid".to_string(),
            as_of_ms: 0,
            balances: vec![AccountBalance {
                account_id: "a1".to_string(),
                account_name: "Empty".to_string(),
                account_type: "asset".to_string(),
                balance_cents: 0,
            }],
            envelopes: vec![],
        };
        let text = snap.to_prompt_text();
        assert!(!text.contains("Empty"));
    }

    #[test]
    fn prompt_text_empty_snapshot_is_clean() {
        let snap = FinancialSnapshot {
            household_id: "hid".to_string(),
            as_of_ms: 0,
            balances: vec![],
            envelopes: vec![],
        };
        let text = snap.to_prompt_text();
        assert!(text.contains("Accounts: none"));
    }
}
