// Financial snapshot builder — T-023
// Delegates balance and envelope queries to core::read (single source of
// truth for balance math). Scheduled transactions are Phase 2.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::core::read::{
    account_balances as read_balances, current_envelope_periods as read_envelopes,
    AccountBalance, EnvelopeStatus,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinancialSnapshot {
    pub household_id: String,
    pub as_of_ms: i64,
    pub balances: Vec<AccountBalance>,
    pub envelopes: Vec<EnvelopeStatus>,
}

impl FinancialSnapshot {
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
                        b.name,
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
                let remaining = e.allocated_cents - e.spent_cents;
                out.push_str(&format!(
                    "  {}: {}/{} ({}% used, {} remaining)\n",
                    e.name,
                    format_dollars(e.spent_cents),
                    format_dollars(e.allocated_cents),
                    pct,
                    format_dollars(remaining)
                ));
            }
        }

        out
    }

    pub fn to_prompt_text_with_ids(&self) -> String {
        let mut out = String::from("=== Financial Snapshot ===\n");

        if self.balances.is_empty() {
            out.push_str("Accounts: none\n");
        } else {
            out.push_str("Accounts (id • name • type • balance)\n");
            for b in &self.balances {
                out.push_str(&format!(
                    "  {} • {} • {} • {}\n",
                    b.id,
                    b.name,
                    b.account_type,
                    format_dollars(b.balance_cents)
                ));
            }
        }

        if !self.envelopes.is_empty() {
            out.push_str("\nEnvelopes (id • name • spent/allocated • remaining)\n");
            for e in &self.envelopes {
                let remaining = e.allocated_cents - e.spent_cents;
                out.push_str(&format!(
                    "  {} • {} • {}/{} • {}\n",
                    e.envelope_id,
                    e.name,
                    format_dollars(e.spent_cents),
                    format_dollars(e.allocated_cents),
                    format_dollars(remaining),
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
    let balances = read_balances(pool, household_id).await?;
    let envelopes = read_envelopes(pool, household_id, as_of_ms).await?;
    Ok(FinancialSnapshot {
        household_id: household_id.to_string(),
        as_of_ms,
        balances,
        envelopes,
    })
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
        let checking_bal = snap.balances.iter().find(|b| b.name == "Checking").unwrap();
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
        let bal = snap.balances.iter().find(|b| b.name == "Checking").unwrap();
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
    }

    #[test]
    fn prompt_text_formats_balance() {
        let snap = FinancialSnapshot {
            household_id: "hid".to_string(),
            as_of_ms: 0,
            balances: vec![AccountBalance {
                id: "a1".to_string(),
                name: "Checking".to_string(),
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
                id: "a1".to_string(),
                name: "Empty".to_string(),
                account_type: "asset".to_string(),
                balance_cents: 0,
            }],
            envelopes: vec![],
        };
        let text = snap.to_prompt_text();
        assert!(!text.contains("Empty"));
    }

    #[test]
    fn prompt_text_with_ids_includes_zero_balance_accounts_and_ids() {
        let snap = FinancialSnapshot {
            household_id: "hid".to_string(),
            as_of_ms: 0,
            balances: vec![
                AccountBalance {
                    id: "acc_chk".to_string(),
                    name: "Checking".to_string(),
                    account_type: "asset".to_string(),
                    balance_cents: 10000,
                },
                AccountBalance {
                    id: "acc_grc".to_string(),
                    name: "Groceries".to_string(),
                    account_type: "expense".to_string(),
                    balance_cents: 0,
                },
            ],
            envelopes: vec![],
        };
        let text = snap.to_prompt_text_with_ids();
        assert!(text.contains("acc_chk"), "expected Checking id: {text}");
        assert!(text.contains("Checking"));
        assert!(text.contains("acc_grc"), "zero-balance expense account must show: {text}");
        assert!(text.contains("Groceries"));
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
