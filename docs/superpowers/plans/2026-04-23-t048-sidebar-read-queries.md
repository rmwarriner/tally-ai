# T-048 Sidebar Read Queries — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the three health-sidebar panels (Accounts, Envelopes, Coming up) to live DB-backed read queries and refresh them automatically after writes.

**Architecture:** New Rust module `core::read` owns three async functions returning Phase-1 read shapes. `ai::snapshot` is refactored to delegate to `core::read` (single source of truth for balance math). Three thin Tauri commands wrap `core::read`. `create_envelope` is extended to seed a current-month `envelope_periods` row using a new `core::envelope::current_month_bounds_ms` helper (IANA zone from `households.timezone`). Frontend uses a shared `useInvalidateSidebar` hook called from `useCommitProposal` on commit success and from every onboarding write step.

**Tech Stack:** Rust (sqlx, chrono, chrono-tz, tokio, thiserror), TypeScript (TanStack Query, React, Vitest), Tauri 2, SQLite/SQLCipher.

**Spec:** `docs/superpowers/specs/2026-04-23-t048-sidebar-read-queries-design.md`

---

## File Structure

**Created:**
- `apps/desktop/src-tauri/src/core/read.rs` — account_balances, current_envelope_periods, coming_up_transactions
- `apps/desktop/src/hooks/useInvalidateSidebar.ts` — shared invalidation hook
- `apps/desktop/src/hooks/useInvalidateSidebar.test.tsx`

**Modified:**
- `Cargo.toml` (workspace) — add `chrono` and `chrono-tz` to `[workspace.dependencies]`
- `apps/desktop/src-tauri/Cargo.toml` — add `chrono` and `chrono-tz` deps
- `apps/desktop/src-tauri/src/core/envelope.rs` — add `current_month_bounds_ms` helper
- `apps/desktop/src-tauri/src/core/mod.rs` — export `read` module
- `apps/desktop/src-tauri/src/ai/snapshot.rs` — delegate `query_balances` / `query_envelopes` to `core::read`
- `apps/desktop/src-tauri/src/commands/mod.rs` — three new commands; extend `create_envelope`
- `apps/desktop/src-tauri/src/lib.rs` — register three new commands
- `apps/desktop/src/hooks/useSidebarData.ts` — rename `PendingTxn` → `ComingUpTxn`, add optional `status`
- `apps/desktop/src/hooks/useCommitProposal.ts` — call invalidation on `committed`
- `apps/desktop/src/hooks/useCommitProposal.test.ts` — new test for invalidation
- `apps/desktop/src/hooks/useOnboardingEngine.ts` — call invalidation after each write step
- `apps/desktop/src/hooks/useOnboardingEngine.test.ts` — new tests for invalidation

---

## Task 1: Add chrono dependencies

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Modify: `apps/desktop/src-tauri/Cargo.toml`

- [ ] **Step 1: Add to workspace deps**

Open `Cargo.toml` at the repo root. Find `[workspace.dependencies]`. Add these two lines (alphabetically placed):

```toml
chrono = { version = "0.4", default-features = false, features = ["clock", "std"] }
chrono-tz = "0.9"
```

- [ ] **Step 2: Add to crate deps**

Open `apps/desktop/src-tauri/Cargo.toml`. Under `[dependencies]`, add:

```toml
chrono.workspace = true
chrono-tz.workspace = true
```

- [ ] **Step 3: Verify the workspace builds**

Run: `cargo build -p tally-desktop 2>&1 | tail -20`
Expected: finishes without error. No test code yet.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml apps/desktop/src-tauri/Cargo.toml Cargo.lock
git commit -m "chore(deps): add chrono and chrono-tz for timezone-aware month bounds"
```

---

## Task 2: `current_month_bounds_ms` helper

**Files:**
- Modify: `apps/desktop/src-tauri/src/core/envelope.rs`
- Test: same file, `#[cfg(test)] mod tests`

- [ ] **Step 1: Write the failing tests**

Replace the entire contents of `apps/desktop/src-tauri/src/core/envelope.rs` with:

```rust
// Envelope budget logic — T-013, T-005, T-048

use chrono::{Datelike, NaiveDate, TimeZone};
use chrono_tz::Tz;

/// Returns (period_start_ms, period_end_ms) for the local calendar month
/// containing `now_ms`, expressed in the household's IANA timezone `tz`.
///
/// Both returned values are unix-milliseconds of the UTC instant that
/// corresponds to **midnight local time** on the first and last days of
/// the month respectively — the same convention used by `transactions.txn_date`.
///
/// Returns `Err` if `tz` is not a valid IANA zone name.
pub fn current_month_bounds_ms(tz: &str, now_ms: i64) -> Result<(i64, i64), String> {
    let tz: Tz = tz.parse().map_err(|e| format!("Invalid timezone '{tz}': {e}"))?;
    let now_utc = chrono::DateTime::from_timestamp_millis(now_ms)
        .ok_or_else(|| format!("Invalid unix-ms timestamp: {now_ms}"))?;
    let now_local = now_utc.with_timezone(&tz);

    let year = now_local.year();
    let month = now_local.month();

    let first = NaiveDate::from_ymd_opt(year, month, 1)
        .ok_or_else(|| format!("Invalid year/month: {year}/{month}"))?;
    let (next_year, next_month) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    let next_first = NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .expect("next month always valid");
    let last = next_first
        .pred_opt()
        .expect("month always has at least one day");

    let start = tz
        .from_local_datetime(&first.and_hms_opt(0, 0, 0).unwrap())
        .single()
        .ok_or_else(|| "Ambiguous local midnight at period start".to_string())?;
    let end = tz
        .from_local_datetime(&last.and_hms_opt(0, 0, 0).unwrap())
        .single()
        .ok_or_else(|| "Ambiguous local midnight at period end".to_string())?;

    Ok((start.timestamp_millis(), end.timestamp_millis()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use chrono_tz::Tz;

    fn ms_from_ymd(tz: Tz, y: i32, m: u32, d: u32) -> i64 {
        tz.with_ymd_and_hms(y, m, d, 0, 0, 0).unwrap().timestamp_millis()
    }

    #[test]
    fn month_bounds_utc_january() {
        let now = ms_from_ymd(chrono_tz::UTC, 2026, 1, 15);
        let (start, end) = current_month_bounds_ms("UTC", now).unwrap();
        assert_eq!(start, ms_from_ymd(chrono_tz::UTC, 2026, 1, 1));
        assert_eq!(end, ms_from_ymd(chrono_tz::UTC, 2026, 1, 31));
    }

    #[test]
    fn month_bounds_utc_december_crosses_year() {
        let now = ms_from_ymd(chrono_tz::UTC, 2025, 12, 20);
        let (start, end) = current_month_bounds_ms("UTC", now).unwrap();
        assert_eq!(start, ms_from_ymd(chrono_tz::UTC, 2025, 12, 1));
        assert_eq!(end, ms_from_ymd(chrono_tz::UTC, 2025, 12, 31));
    }

    #[test]
    fn month_bounds_chicago_differs_from_utc() {
        // 2026-03-15T05:30:00Z is 2026-03-15 00:30 in Chicago.
        let now = chrono::Utc
            .with_ymd_and_hms(2026, 3, 15, 5, 30, 0)
            .unwrap()
            .timestamp_millis();
        let chi: Tz = "America/Chicago".parse().unwrap();
        let (start, _end) = current_month_bounds_ms("America/Chicago", now).unwrap();
        assert_eq!(start, ms_from_ymd(chi, 2026, 3, 1));
    }

    #[test]
    fn month_bounds_tokyo() {
        // 2026-01-31T20:00:00Z is 2026-02-01 05:00 in Tokyo — month must be Feb.
        let now = chrono::Utc
            .with_ymd_and_hms(2026, 1, 31, 20, 0, 0)
            .unwrap()
            .timestamp_millis();
        let tyo: Tz = "Asia/Tokyo".parse().unwrap();
        let (start, _end) = current_month_bounds_ms("Asia/Tokyo", now).unwrap();
        assert_eq!(start, ms_from_ymd(tyo, 2026, 2, 1));
    }

    #[test]
    fn month_bounds_february_leap_year() {
        let now = ms_from_ymd(chrono_tz::UTC, 2024, 2, 10);
        let (_start, end) = current_month_bounds_ms("UTC", now).unwrap();
        assert_eq!(end, ms_from_ymd(chrono_tz::UTC, 2024, 2, 29));
    }

    #[test]
    fn month_bounds_dst_spring_forward_chicago() {
        // 2026-03-08T03:00:00 local Chicago is after spring-forward.
        // The test simply asserts that the function returns bounds (no ambiguity error)
        // for a timestamp during DST.
        let chi: Tz = "America/Chicago".parse().unwrap();
        let now = chi
            .with_ymd_and_hms(2026, 3, 10, 12, 0, 0)
            .unwrap()
            .timestamp_millis();
        let res = current_month_bounds_ms("America/Chicago", now);
        assert!(res.is_ok(), "expected Ok, got {:?}", res);
    }

    #[test]
    fn month_bounds_invalid_tz_errors() {
        let res = current_month_bounds_ms("Not/A_Zone", 0);
        assert!(res.is_err());
    }
}
```

- [ ] **Step 2: Run tests and verify they fail to build (no helper yet)**

Run: `cargo test -p tally-desktop --lib core::envelope 2>&1 | tail -15`
Expected: compile error because the module imports aren't wired yet — that's fine, the helper body is written together with the tests in this step.

- [ ] **Step 3: Verify tests pass**

Run: `cargo test -p tally-desktop --lib core::envelope 2>&1 | tail -20`
Expected: `test result: ok. 7 passed; 0 failed`

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src-tauri/src/core/envelope.rs
git commit -m "feat(core): add current_month_bounds_ms helper (T-048)"
```

---

## Task 3: `core::read::account_balances`

**Files:**
- Create: `apps/desktop/src-tauri/src/core/read.rs`
- Modify: `apps/desktop/src-tauri/src/core/mod.rs`

- [ ] **Step 1: Export the new module**

Open `apps/desktop/src-tauri/src/core/mod.rs`. Add `pub mod read;` in alphabetical position among the other `pub mod` lines.

- [ ] **Step 2: Write `core::read` scaffolding with failing tests for `account_balances`**

Create `apps/desktop/src-tauri/src/core/read.rs` with:

```rust
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
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p tally-desktop --lib core::read 2>&1 | tail -20`
Expected: `test result: ok. 5 passed; 0 failed`

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src-tauri/src/core/read.rs apps/desktop/src-tauri/src/core/mod.rs
git commit -m "feat(core): add core::read::account_balances (T-048)"
```

---

## Task 4: `core::read::current_envelope_periods`

**Files:**
- Modify: `apps/desktop/src-tauri/src/core/read.rs`

- [ ] **Step 1: Add the failing tests**

Append to `apps/desktop/src-tauri/src/core/read.rs`, inside `mod tests`:

```rust
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
        // Period is in the past relative to as_of.
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
```

- [ ] **Step 2: Add the implementation**

Append this to `core/read.rs` (above the `#[cfg(test)]` block):

```rust
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
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p tally-desktop --lib core::read 2>&1 | tail -20`
Expected: `test result: ok. 9 passed; 0 failed`

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src-tauri/src/core/read.rs
git commit -m "feat(core): add core::read::current_envelope_periods (T-048)"
```

---

## Task 5: `core::read::coming_up_transactions`

**Files:**
- Modify: `apps/desktop/src-tauri/src/core/read.rs`

- [ ] **Step 1: Add the failing tests**

Append to `mod tests` in `core/read.rs`:

```rust
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
```

- [ ] **Step 2: Add the implementation**

Append above the `#[cfg(test)]` block in `core/read.rs`:

```rust
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
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p tally-desktop --lib core::read 2>&1 | tail -20`
Expected: `test result: ok. 13 passed; 0 failed`

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src-tauri/src/core/read.rs
git commit -m "feat(core): add core::read::coming_up_transactions (T-048)"
```

---

## Task 6: Refactor `ai::snapshot` to delegate to `core::read`

**Files:**
- Modify: `apps/desktop/src-tauri/src/ai/snapshot.rs`

- [ ] **Step 1: Replace snapshot's balance/envelope helpers with delegating calls**

Open `apps/desktop/src-tauri/src/ai/snapshot.rs`.

Delete the existing top-level `AccountBalance` and `EnvelopeHealth` structs (lines ~8–26), the `build_snapshot` body's helpers `query_balances` (lines ~141–183), `query_envelopes` (lines ~194–232), and the `BalanceRow` / `EnvelopeRow` structs. Keep `FinancialSnapshot`, `to_prompt_text`, `to_prompt_text_with_ids`, `format_dollars`, and the existing tests.

Then add imports at the top of the file (replacing existing imports block):

```rust
// Financial snapshot builder — T-023
// Delegates balance and envelope queries to core::read (single source of
// truth for balance math). Scheduled transactions are Phase 2.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::core::read::{
    account_balances as read_balances, current_envelope_periods as read_envelopes,
    AccountBalance, EnvelopeStatus,
};
```

Re-export `AccountBalance` at the top (after the imports) for any existing external users:

```rust
// Re-exports so callers that imported these from ai::snapshot keep working.
pub use crate::core::read::AccountBalance as AccountBalanceExport;
```

Actually — don't alias. Instead, change `FinancialSnapshot` to carry the `core::read` types directly, and update the two `to_prompt_text*` methods to use the new field names (`b.id`, `b.name`, `b.account_type`, `b.balance_cents` — `balance_cents` is unchanged; `account_id` becomes `id`, `account_name` becomes `name`; `envelope_name` becomes `name`).

Also: the snapshot's `EnvelopeHealth` exposed `remaining_cents` and `period_end_ms`. The `EnvelopeStatus` from `core::read` does not. Since `to_prompt_text*` only uses `allocated_cents`, `spent_cents`, and computes `remaining = allocated - spent` inline, we can drop `remaining_cents`. `period_end_ms` is not used in any prompt output — grep confirms it. Drop it too.

The final file body should be:

```rust
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
```

- [ ] **Step 2: Check for external consumers of removed types**

Run: `grep -rn "ai::snapshot::AccountBalance\|ai::snapshot::EnvelopeHealth\|AccountBalance\s*{" apps/desktop/src-tauri/src 2>&1 | grep -v snapshot.rs | grep -v core/read.rs`

Expected: empty output. If any file references `EnvelopeHealth` or the old `AccountBalance` fields (`account_id`, `account_name`), update them to the new field names (`id`, `name`) and import from `crate::core::read`.

- [ ] **Step 3: Run the full test suite**

Run: `cargo test -p tally-desktop --lib 2>&1 | tail -10`
Expected: `test result: ok. 220+ passed; 0 failed` — snapshot tests plus new `core::read` tests, nothing broken.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src-tauri/src/ai/snapshot.rs
git commit -m "refactor(ai): snapshot delegates balance/envelope queries to core::read (T-048)"
```

---

## Task 7: Extend `create_envelope` to seed a current-month period

**Files:**
- Modify: `apps/desktop/src-tauri/src/commands/mod.rs`

- [ ] **Step 1: Write a new integration test**

Tests in `commands/mod.rs` would require full Tauri state, which is heavy. Instead, add a test of the DB-side behavior to `apps/desktop/src-tauri/src/core/envelope.rs` as a sibling helper + unit test. But `create_envelope` itself is the Tauri command — the simplest place to unit-test the "also inserts a period row" behavior is to factor the DB-mutating body into a testable helper.

Create a pure DB helper in `apps/desktop/src-tauri/src/core/envelope.rs` (append below `current_month_bounds_ms` and above `#[cfg(test)]`):

```rust
use sqlx::SqlitePool;

/// Inserts a new envelope and its current-month envelope_periods row.
/// Returns the new envelope ULID. Resolves or creates an expense account
/// for the envelope name (same behavior as before). Month bounds come from
/// `current_month_bounds_ms` using the household's IANA `timezone`.
pub async fn create_envelope_with_current_period(
    pool: &SqlitePool,
    household_id: &str,
    name: &str,
    now_ms: i64,
) -> Result<String, String> {
    use crate::id::new_ulid;

    // Look up the household timezone.
    let tz: (String,) =
        sqlx::query_as("SELECT timezone FROM households WHERE id = ?")
            .bind(household_id)
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;

    let (period_start, period_end) = current_month_bounds_ms(&tz.0, now_ms)?;

    // Resolve the target expense account: pick the first non-placeholder
    // expense account, or create a generic one under the Expenses placeholder.
    let expense_account: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM accounts WHERE household_id = ? AND type = 'expense' AND is_placeholder = 0 LIMIT 1",
    )
    .bind(household_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let account_id = if let Some((id,)) = expense_account {
        id
    } else {
        let id = new_ulid();
        let parent: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM accounts WHERE household_id = ? AND type = 'expense' AND is_placeholder = 1 AND name = 'Expenses' LIMIT 1",
        )
        .bind(household_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;

        sqlx::query(
            "INSERT INTO accounts (id, household_id, parent_id, name, type, normal_balance, is_placeholder, currency, created_at)
             VALUES (?, ?, ?, ?, 'expense', 'debit', 0, 'USD', ?)",
        )
        .bind(&id)
        .bind(household_id)
        .bind(parent.map(|(pid,)| pid))
        .bind(name)
        .bind(now_ms)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

        id
    };

    let envelope_id = new_ulid();
    sqlx::query(
        "INSERT INTO envelopes (id, household_id, account_id, name, created_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&envelope_id)
    .bind(household_id)
    .bind(&account_id)
    .bind(name)
    .bind(now_ms)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    sqlx::query(
        "INSERT INTO envelope_periods
           (id, envelope_id, period_start, period_end, allocated, spent, created_at)
         VALUES (?, ?, ?, ?, 0, 0, ?)",
    )
    .bind(new_ulid())
    .bind(&envelope_id)
    .bind(period_start)
    .bind(period_end)
    .bind(now_ms)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(envelope_id)
}
```

Then add a test inside the existing `#[cfg(test)]` block:

```rust
    #[tokio::test]
    async fn create_envelope_with_current_period_inserts_period_row() {
        use crate::db::connection::create_encrypted_db;
        use crate::db::migrations::run_migrations;
        use crate::id::new_ulid;
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let path = dir.path().join("env_test.db");
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

        let now = chrono::Utc
            .with_ymd_and_hms(2026, 4, 15, 12, 0, 0)
            .unwrap()
            .timestamp_millis();

        let env_id = create_envelope_with_current_period(&pool, &hid, "Groceries", now)
            .await
            .unwrap();

        let (start, end): (i64, i64) = sqlx::query_as(
            "SELECT period_start, period_end FROM envelope_periods WHERE envelope_id = ?",
        )
        .bind(&env_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        let utc: chrono_tz::Tz = "UTC".parse().unwrap();
        let expect_start = utc.with_ymd_and_hms(2026, 4, 1, 0, 0, 0).unwrap().timestamp_millis();
        let expect_end = utc.with_ymd_and_hms(2026, 4, 30, 0, 0, 0).unwrap().timestamp_millis();
        assert_eq!(start, expect_start);
        assert_eq!(end, expect_end);
    }
```

- [ ] **Step 2: Replace the body of `create_envelope` command to call the helper**

Open `apps/desktop/src-tauri/src/commands/mod.rs`. Replace the entire body of the `create_envelope` command (from `#[derive(Deserialize)] pub struct CreateEnvelopeArgs` down through the `Ok(envelope_id)` close of the `create_envelope` fn) with:

```rust
#[derive(Deserialize)]
pub struct CreateEnvelopeArgs {
    pub name: String,
}

/// Creates a new envelope and seeds a current-month envelope_periods row.
/// Returns the new envelope ULID.
#[tauri::command]
pub async fn create_envelope(
    state: State<'_, AppState>,
    args: CreateEnvelopeArgs,
) -> Result<String, String> {
    let pool = state.pool.lock().expect("pool lock").clone().ok_or("Database not open")?;
    let household_id = state
        .household_id
        .lock()
        .expect("household_id lock")
        .clone()
        .ok_or("Household not set")?;

    crate::core::envelope::create_envelope_with_current_period(
        &pool,
        &household_id,
        &args.name,
        now_ms(),
    )
    .await
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p tally-desktop --lib core::envelope 2>&1 | tail -15`
Expected: `test result: ok. 8 passed; 0 failed`

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src-tauri/src/core/envelope.rs apps/desktop/src-tauri/src/commands/mod.rs
git commit -m "feat(core): create_envelope seeds current-month envelope_periods (T-048)"
```

---

## Task 8: Three new Tauri commands

**Files:**
- Modify: `apps/desktop/src-tauri/src/commands/mod.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Add the three new commands**

In `commands/mod.rs`, add at the end of the file (after the existing commands):

```rust
// ── Sidebar read queries (T-048) ──────────────────────────────────────────────

use crate::core::read::{
    account_balances as read_balances, coming_up_transactions as read_coming_up,
    current_envelope_periods as read_envelopes, AccountBalance, ComingUpTxn, EnvelopeStatus,
};

#[tauri::command]
pub async fn get_account_balances(
    state: State<'_, AppState>,
) -> Result<Vec<AccountBalance>, String> {
    let pool = state.pool.lock().expect("pool lock").clone().ok_or("Database not open")?;
    let household_id = state
        .household_id
        .lock()
        .expect("household_id lock")
        .clone()
        .ok_or("Household not set")?;

    read_balances(&pool, &household_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_current_envelope_periods(
    state: State<'_, AppState>,
) -> Result<Vec<EnvelopeStatus>, String> {
    let pool = state.pool.lock().expect("pool lock").clone().ok_or("Database not open")?;
    let household_id = state
        .household_id
        .lock()
        .expect("household_id lock")
        .clone()
        .ok_or("Household not set")?;

    read_envelopes(&pool, &household_id, now_ms())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_pending_transactions(
    state: State<'_, AppState>,
) -> Result<Vec<ComingUpTxn>, String> {
    let pool = state.pool.lock().expect("pool lock").clone().ok_or("Database not open")?;
    let household_id = state
        .household_id
        .lock()
        .expect("household_id lock")
        .clone()
        .ok_or("Household not set")?;

    read_coming_up(&pool, &household_id, now_ms(), 50)
        .await
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Register the commands in `lib.rs`**

Open `apps/desktop/src-tauri/src/lib.rs`. In the `tauri::generate_handler!` list, add three lines (alphabetical placement is fine; at end is also fine):

```rust
            commands::get_account_balances,
            commands::get_current_envelope_periods,
            commands::get_pending_transactions,
```

- [ ] **Step 3: Verify the full Rust build**

Run: `cargo build -p tally-desktop 2>&1 | tail -10`
Expected: compiles clean. No warnings about unused commands.

- [ ] **Step 4: Run the full test suite**

Run: `cargo test -p tally-desktop --lib 2>&1 | tail -10`
Expected: `test result: ok. 220+ passed; 0 failed`

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/commands/mod.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(commands): register sidebar read-query Tauri commands (T-048)"
```

---

## Task 9: Frontend `useInvalidateSidebar` hook

**Files:**
- Create: `apps/desktop/src/hooks/useInvalidateSidebar.ts`
- Create: `apps/desktop/src/hooks/useInvalidateSidebar.test.tsx`

- [ ] **Step 1: Write the failing test**

Create `apps/desktop/src/hooks/useInvalidateSidebar.test.tsx`:

```tsx
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, act } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";

import { useInvalidateSidebar } from "./useInvalidateSidebar";

describe("useInvalidateSidebar", () => {
  it("invalidates queries under the 'sidebar' root key", async () => {
    const queryClient = new QueryClient();
    const spy = vi.spyOn(queryClient, "invalidateQueries");

    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    );

    const { result } = renderHook(() => useInvalidateSidebar(), { wrapper });

    await act(async () => {
      await result.current();
    });

    expect(spy).toHaveBeenCalledWith({ queryKey: ["sidebar"] });
  });
});
```

- [ ] **Step 2: Run the test and watch it fail**

Run: `pnpm --filter tally-desktop test run useInvalidateSidebar 2>&1 | tail -15`
Expected: FAIL with "Cannot find module './useInvalidateSidebar'".

- [ ] **Step 3: Implement the hook**

Create `apps/desktop/src/hooks/useInvalidateSidebar.ts`:

```ts
import { useQueryClient } from "@tanstack/react-query";
import { useCallback } from "react";

export function useInvalidateSidebar() {
  const queryClient = useQueryClient();
  return useCallback(
    () => queryClient.invalidateQueries({ queryKey: ["sidebar"] }),
    [queryClient],
  );
}
```

- [ ] **Step 4: Run the test and watch it pass**

Run: `pnpm --filter tally-desktop test run useInvalidateSidebar 2>&1 | tail -10`
Expected: `Tests  1 passed (1)`.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/hooks/useInvalidateSidebar.ts apps/desktop/src/hooks/useInvalidateSidebar.test.tsx
git commit -m "feat(ui): add useInvalidateSidebar hook (T-048)"
```

---

## Task 10: Rename `PendingTxn` → `ComingUpTxn` in `useSidebarData`

**Files:**
- Modify: `apps/desktop/src/hooks/useSidebarData.ts`

- [ ] **Step 1: Update the type and export**

Open `apps/desktop/src/hooks/useSidebarData.ts`. Replace the `PendingTxn` interface with:

```ts
export interface ComingUpTxn {
  id: string;
  txn_date: number;
  status?: "pending" | "posted";
  payee?: string;
  memo?: string;
  amount_cents: number;
}

// Back-compat alias; remove once no callers reference PendingTxn.
export type PendingTxn = ComingUpTxn;
```

Change the return type of `usePendingTransactions`:

```ts
export function usePendingTransactions() {
  return useQuery({
    queryKey: ["sidebar", "pending"],
    queryFn: async () => invoke<ComingUpTxn[]>("get_pending_transactions"),
    staleTime: 10_000,
  });
}
```

- [ ] **Step 2: Run sidebar tests**

Run: `pnpm --filter tally-desktop test run useSidebarData 2>&1 | tail -10`
Expected: PASS. The `ComingUpPanel` tests also pass unchanged.

- [ ] **Step 3: Run the full frontend test suite**

Run: `pnpm --filter tally-desktop test 2>&1 | tail -10`
Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/hooks/useSidebarData.ts
git commit -m "refactor(ui): rename PendingTxn to ComingUpTxn with back-compat alias (T-048)"
```

---

## Task 11: Invalidate sidebar on `commit_proposal` success

**Files:**
- Modify: `apps/desktop/src/hooks/useCommitProposal.ts`
- Modify: `apps/desktop/src/hooks/useCommitProposal.test.ts`

- [ ] **Step 1: Add the failing test**

Open `apps/desktop/src/hooks/useCommitProposal.test.ts`. The existing test file uses `renderHook(() => useCommitProposal({ invoke: ... }))` directly, without a `QueryClientProvider`. Wrap all `renderHook` calls in a `QueryClientProvider` and add a new test. The full file should look like this after changes (replace existing content):

```ts
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, act } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";

import { useChatStore } from "../stores/chatStore";
import type { TransactionProposal } from "../components/chat/chatTypes";
import { useCommitProposal } from "./useCommitProposal";

function makeWrapper() {
  const queryClient = new QueryClient();
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  return { queryClient, wrapper };
}

const proposal: TransactionProposal = {
  txn_date_ms: 0,
  lines: [
    { account_id: "a1", amount_cents: 100, side: "debit" },
    { account_id: "a2", amount_cents: 100, side: "credit" },
  ],
};

beforeEach(() => {
  useChatStore.setState({ messages: [] });
  useChatStore.getState().addLocalMessage({
    kind: "transaction",
    id: "msg1",
    ts: 0,
    transaction_id: "t1",
    state: "pending",
    transaction: {
      id: "t1",
      payee: "",
      txn_date: 0,
      amount_cents: 100,
      account_name: "a1",
      lines: [],
    },
    proposal,
  });
});

describe("useCommitProposal", () => {
  it("flips the card to posted on success", async () => {
    const invoke = vi.fn().mockResolvedValue({ status: "committed", txn_id: "t2" });
    const { wrapper } = makeWrapper();
    const { result } = renderHook(
      () => useCommitProposal({ invoke: invoke as never }),
      { wrapper },
    );

    await act(async () => {
      await result.current.commit("msg1", proposal);
    });

    const msg = useChatStore.getState().messages[0];
    expect(msg.kind === "transaction" && msg.state === "posted").toBe(true);
  });

  it("invalidates the sidebar queries on successful commit", async () => {
    const invoke = vi.fn().mockResolvedValue({ status: "committed", txn_id: "t2" });
    const { queryClient, wrapper } = makeWrapper();
    const spy = vi.spyOn(queryClient, "invalidateQueries");

    const { result } = renderHook(
      () => useCommitProposal({ invoke: invoke as never }),
      { wrapper },
    );

    await act(async () => {
      await result.current.commit("msg1", proposal);
    });

    expect(spy).toHaveBeenCalledWith({ queryKey: ["sidebar"] });
  });

  it("does not invalidate on rejection", async () => {
    const invoke = vi.fn().mockResolvedValue({
      status: "rejected",
      validation: { status: "REJECTED", errors: [{ user_message: "bad" }] },
    });
    const { queryClient, wrapper } = makeWrapper();
    const spy = vi.spyOn(queryClient, "invalidateQueries");

    const { result } = renderHook(
      () => useCommitProposal({ invoke: invoke as never }),
      { wrapper },
    );

    await act(async () => {
      await result.current.commit("msg1", proposal);
    });

    expect(spy).not.toHaveBeenCalled();
  });

  it("surfaces the error message on invoke failure", async () => {
    const invoke = vi.fn().mockRejectedValue(new Error("network down"));
    const { wrapper } = makeWrapper();
    const { result } = renderHook(
      () => useCommitProposal({ invoke: invoke as never }),
      { wrapper },
    );

    await act(async () => {
      await result.current.commit("msg1", proposal);
    });

    const msg = useChatStore.getState().messages[0];
    expect(msg.kind === "transaction" && msg.commit_error).toBe("network down");
  });
});
```

> Note: the above replaces the existing tests file entirely. If the existing file has additional tests beyond the four shown here, preserve them by adding the `makeWrapper` pattern and the two new invalidation tests (`invalidates the sidebar queries on successful commit`, `does not invalidate on rejection`) without deleting pre-existing tests. Check what's there with `cat apps/desktop/src/hooks/useCommitProposal.test.ts` before overwriting.

- [ ] **Step 2: Run the test and watch the new assertions fail**

Run: `pnpm --filter tally-desktop test run useCommitProposal 2>&1 | tail -20`
Expected: the "invalidates the sidebar queries on successful commit" test fails.

- [ ] **Step 3: Wire the invalidation into the hook**

Open `apps/desktop/src/hooks/useCommitProposal.ts`. Add the import at the top:

```ts
import { useInvalidateSidebar } from "./useInvalidateSidebar";
```

Inside `useCommitProposal`, after the existing `useChatStore` lines, add:

```ts
  const invalidateSidebar = useInvalidateSidebar();
```

In the `committed` branch of the `commit` callback (right after the existing `updateMessage` call), add:

```ts
        void invalidateSidebar();
```

Add `invalidateSidebar` to the `commit` callback's dependency array.

- [ ] **Step 4: Run the test and watch it pass**

Run: `pnpm --filter tally-desktop test run useCommitProposal 2>&1 | tail -15`
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/hooks/useCommitProposal.ts apps/desktop/src/hooks/useCommitProposal.test.ts
git commit -m "feat(ui): invalidate sidebar on commit success (T-048)"
```

---

## Task 12: Invalidate sidebar after onboarding write steps

**Files:**
- Modify: `apps/desktop/src/hooks/useOnboardingEngine.ts`
- Modify: `apps/desktop/src/hooks/useOnboardingEngine.test.ts`

- [ ] **Step 1: Write the failing test**

Open `apps/desktop/src/hooks/useOnboardingEngine.test.ts`. Find where test setup passes `deps` to `buildOnboardingHandler` (or its exported helpers). Inject an additional `invalidateSidebar: vi.fn()` dep, and add a new test block near the end of the file:

```ts
describe("sidebar invalidation", () => {
  it("calls invalidateSidebar after each DB write", async () => {
    const invalidateSidebar = vi.fn();
    const mockInvoke = vi.fn()
      .mockResolvedValueOnce(false)                            // check_setup_status
      .mockResolvedValueOnce("hh_01")                          // create_household
      .mockResolvedValueOnce("ac_01")                          // create_account
      .mockResolvedValueOnce(undefined)                        // set_opening_balance
      .mockResolvedValueOnce("en_01");                         // create_envelope

    const handler = buildOnboardingHandler({
      addSystemMessage: vi.fn(),
      addSetupCard: vi.fn(),
      addHandoffMessage: vi.fn(),
      invoke: mockInvoke as never,
      invalidateSidebar,
    });

    await handler.checkAndStart();
    await handler.handleInput("fresh");
    await handler.handleInput("Smith Family");
    await handler.handleInput("America/Chicago");
    await handler.handleInput("correcthorsebatterystaple");
    await handler.handleInput("Checking");
    await handler.handleInput("1000");
    await handler.handleInput("done");
    await handler.handleInput("Groceries");
    await handler.handleInput("done");

    // Expect one invalidation per write: create_household, create_account,
    // set_opening_balance, create_envelope.
    expect(invalidateSidebar).toHaveBeenCalledTimes(4);
  });
});
```

> The exact sequence of `handler.handleInput()` calls above should mirror the existing "walks the fresh path" test in the same file. If the existing test uses a different sequence (different timezone format, different account type prompt, etc.), match it. The critical assertion is that `invalidateSidebar` fires exactly once per write — 4 times for the fresh-path flow (household, account, opening_balance, envelope). Read the adjacent passing tests before writing this one.

- [ ] **Step 2: Run the test and watch it fail**

Run: `pnpm --filter tally-desktop test run useOnboardingEngine 2>&1 | tail -20`
Expected: new test fails because `invalidateSidebar` is unknown prop and never called.

- [ ] **Step 3: Wire invalidation into the engine**

Open `apps/desktop/src/hooks/useOnboardingEngine.ts`.

Add to `OnboardingDeps`:

```ts
  invalidateSidebar: () => void | Promise<void>;
```

Add to the `useOnboarding` hook's default deps at the bottom of the file, alongside `invoke: tauriInvoke`:

```ts
import { useInvalidateSidebar } from "./useInvalidateSidebar";
// ...
export function useOnboarding(...) {
  const invalidateSidebar = useInvalidateSidebar();
  // ... pass as part of deps
}
```

Find the actual hook export at the bottom of the file (`export function useOnboarding(...)` or similar) and update the `buildOnboardingHandler` invocation to include `invalidateSidebar`.

Inside `buildOnboardingHandler`, immediately after each of the following `await deps.invoke(...)` calls, add `void deps.invalidateSidebar();`:

1. After `create_household` (line ~116)
2. After `create_account` (line ~161)
3. After `set_opening_balance` (line ~165)
4. After `create_envelope` (line ~209)
5. After `import_hledger` (line ~283)

Example for the `create_household` site:

```ts
const id = await deps.invoke<string>("create_household", { args: { ... } });
void deps.invalidateSidebar();
```

- [ ] **Step 4: Verify all onboarding tests still pass**

Run: `pnpm --filter tally-desktop test run useOnboardingEngine 2>&1 | tail -20`
Expected: all tests pass (existing + new). If the old tests break because they don't pass `invalidateSidebar`, update their `deps` object to include `invalidateSidebar: vi.fn()` (no assertions — just satisfy the type).

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/hooks/useOnboardingEngine.ts apps/desktop/src/hooks/useOnboardingEngine.test.ts
git commit -m "feat(ui): invalidate sidebar after onboarding DB writes (T-048)"
```

---

## Task 13: Full build + coverage check

**Files:** none — verification only.

- [ ] **Step 1: Run the full Rust test suite**

Run: `cargo test -p tally-desktop --lib 2>&1 | tail -10`
Expected: `test result: ok. ~225 passed; 0 failed`.

- [ ] **Step 2: Run the full frontend test suite with coverage**

Run: `pnpm --filter tally-desktop test --coverage 2>&1 | tail -25`
Expected: all tests pass; overall coverage ≥ 80% (pre-commit threshold).

- [ ] **Step 3: Smoke-test the app**

Run: `pnpm --filter tally-desktop tauri dev` in one terminal.

Verify manually:

1. Start fresh (delete `~/Library/Application Support/tally-desktop/tally.db` if present).
2. Complete onboarding: enter household name, timezone, passphrase, one account with an opening balance, one envelope.
3. Sidebar should show the account balance and the envelope (allocated $0.00, spent $0.00) immediately — not after a 10-second stale window.
4. Record a transaction via chat that posts cleanly. After "Confirm" on the card, the account balance in the sidebar should update without any manual refresh.

- [ ] **Step 4: Update memory**

Edit `/Users/robert/.claude/projects/-Users-robert-Projects-tally-ai/memory/project_tally_ai.md`: move T-048 from "Next tickets" to "Completed tickets (as of 2026-04-23)" and update the completed list with a one-line description.

- [ ] **Step 5: Final commit (if memory was changed)**

Memory lives outside the repo — no commit needed.

---

## Spec Coverage Self-Review

Cross-checking against `docs/superpowers/specs/2026-04-23-t048-sidebar-read-queries-design.md`:

| Spec requirement | Implementing task |
|---|---|
| `get_account_balances` Tauri command | Task 8 |
| `get_current_envelope_periods` Tauri command | Task 8 |
| `get_pending_transactions` Tauri command | Task 8 |
| New `core::read` module | Tasks 3, 4, 5 |
| Refactor `ai::snapshot` to delegate | Task 6 |
| `create_envelope` seeds envelope_periods | Task 7 |
| LEFT JOIN for envelopes without a period | Task 4 |
| Coming-up = pending ∪ future-posted, expense→asset amount fallback | Task 5 |
| `current_month_bounds_ms` helper with DST / zone tests | Task 2 |
| `PendingTxn` → `ComingUpTxn` rename with optional `status` | Task 10 |
| `useInvalidateSidebar` hook | Task 9 |
| Invalidate on `commit_proposal` success | Task 11 |
| Invalidate on onboarding writes | Task 12 |
| 80%+ coverage, all tests pass | Task 13 |
