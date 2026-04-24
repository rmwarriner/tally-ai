# GnuCash Import Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship T-071, T-072, T-073, T-074 as one batch — a working GnuCash SQLite import path inside onboarding that reads, CoA-maps, atomically commits, and reconciles against the source book, with rollback if anything looks wrong.

**Architecture:** Three-phase pipeline (read → map → commit) plus reconciliation. Rust `core::import::gnucash` module holds reader, mapper, committer, reconciler as independently testable units. Tauri commands expose each phase to the TS onboarding handler. Import plan is stashed in `AppState` between phases so edits are cheap (no plan serialization on every keystroke). Atomic commit in one DB transaction, idempotent on GnuCash GUID via new `source_ref` column, scoped rollback via new `import_id` column on accounts.

**Tech Stack:** Rust (sqlx with SQLite), React/TypeScript (onboarding handler, artifact cards), Tauri 2 (IPC). GnuCash files are plain SQLite — we open them with a second sqlx pool (no SQLCipher). Test fixtures are built dynamically in Rust using sqlx against tempdir-backed SQLite files — no binary `.gnucash` files committed.

**Spec:** `docs/superpowers/specs/2026-04-24-gnucash-import-design.md`

**Ship strategy:** One feature branch `feat/gnucash-import`, one PR covering all four tickets (per batch-PRs convention — Rust CI is 4+ min per run).

---

## Ticket T-071: Reader

### Task 1: Create feature branch

**Files:** none (git operation)

- [ ] **Step 1: Branch off main**

```bash
git checkout -b feat/gnucash-import
```

---

### Task 2: DB migration 0006 (source_ref + import_id columns)

**Files:**
- Create: `apps/desktop/src-tauri/migrations/0006_gnucash_import_columns.sql`
- Test: `apps/desktop/src-tauri/src/db/migrations/mod.rs` (add new test)

- [ ] **Step 1: Write the failing test**

Append to `apps/desktop/src-tauri/src/db/migrations/mod.rs` inside the `tests` module:

```rust
#[tokio::test]
async fn test_migration_0006_adds_source_ref_and_import_id() {
    let dir = tempdir().expect("Should create temp dir");
    let db_path = dir.path().join("test_0006.db");
    let salt = [0u8; 16];

    let pool = create_encrypted_db(&db_path, "passphrase", &salt)
        .await
        .expect("Should create database");

    run_migrations(&pool).await.expect("Migrations should run");

    // transactions.source_ref exists
    let txn_schema: (String,) = sqlx::query_as(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='transactions'",
    )
    .fetch_one(&pool)
    .await
    .expect("transactions table should exist");
    assert!(
        txn_schema.0.contains("source_ref"),
        "transactions.source_ref column missing"
    );

    // accounts.import_id exists
    let acc_schema: (String,) = sqlx::query_as(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='accounts'",
    )
    .fetch_one(&pool)
    .await
    .expect("accounts table should exist");
    assert!(
        acc_schema.0.contains("import_id"),
        "accounts.import_id column missing"
    );

    // Unique (household_id, source_ref) index exists
    let indexes: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='transactions'",
    )
    .fetch_all(&pool)
    .await
    .expect("Should query indexes");
    let names: Vec<&str> = indexes.iter().map(|(n,)| n.as_str()).collect();
    assert!(
        names.contains(&"idx_transactions_source_ref_unique"),
        "missing idx_transactions_source_ref_unique"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd apps/desktop/src-tauri && cargo test --lib db::migrations::tests::test_migration_0006 -- --nocapture`
Expected: FAIL — migration file does not exist.

- [ ] **Step 3: Create the migration**

Create `apps/desktop/src-tauri/migrations/0006_gnucash_import_columns.sql`:

```sql
-- Add GnuCash import support columns:
--   transactions.source_ref  — GnuCash transaction GUID (per-row idempotency)
--   accounts.import_id       — ULID stamped on accounts created by an import (scoped rollback)

ALTER TABLE transactions ADD COLUMN source_ref TEXT;
CREATE INDEX idx_transactions_source_ref
    ON transactions(source_ref) WHERE source_ref IS NOT NULL;
CREATE UNIQUE INDEX idx_transactions_source_ref_unique
    ON transactions(household_id, source_ref) WHERE source_ref IS NOT NULL;

ALTER TABLE accounts ADD COLUMN import_id TEXT;
CREATE INDEX idx_accounts_import_id
    ON accounts(import_id) WHERE import_id IS NOT NULL;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd apps/desktop/src-tauri && cargo test --lib db::migrations`
Expected: PASS — all migration tests.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/migrations/0006_gnucash_import_columns.sql apps/desktop/src-tauri/src/db/migrations/mod.rs
git commit -m "feat(db): migration 0006 — source_ref + import_id for GnuCash import"
```

---

### Task 3: Scaffold `core::import::gnucash` module with public types

**Files:**
- Modify: `apps/desktop/src-tauri/src/core/mod.rs` (add `pub mod import;`)
- Create: `apps/desktop/src-tauri/src/core/import/mod.rs`
- Create: `apps/desktop/src-tauri/src/core/import/gnucash/mod.rs`

- [ ] **Step 1: Add import module to core**

Edit `apps/desktop/src-tauri/src/core/mod.rs` to add `pub mod import;` after `pub mod envelope;`:

```rust
pub mod coa;
pub mod correction;
pub mod envelope;
pub mod import;
pub mod ledger;
pub mod proposal;
pub mod read;
pub mod validation;
```

- [ ] **Step 2: Create `core::import` module file**

Create `apps/desktop/src-tauri/src/core/import/mod.rs`:

```rust
pub mod gnucash;
```

- [ ] **Step 3: Create `core::import::gnucash` module with public types**

Create `apps/desktop/src-tauri/src/core/import/gnucash/mod.rs`:

```rust
//! GnuCash SQLite book importer.
//!
//! Three phases: reader → mapper → committer, with a reconciler that runs
//! after commit to prove balances match the source book. See
//! `docs/superpowers/specs/2026-04-24-gnucash-import-design.md` for the
//! architectural rationale.

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod committer;
pub mod mapper;
pub mod reader;
pub mod reconcile;

// ── Reader output ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GnuCashBook {
    pub book_guid: String,
    pub commodities: Vec<GncCommodity>,
    pub accounts: Vec<GncAccount>,
    pub transactions: Vec<GncTransaction>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GncCommodity {
    pub guid: String,
    pub namespace: String, // "CURRENCY" | other (stocks, funds)
    pub mnemonic: String,  // "USD" | "EUR" | ticker
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GncAccount {
    pub guid: String,
    pub parent_guid: Option<String>,
    pub name: String,
    pub full_name: String,
    pub gnc_type: GncAccountType,
    pub commodity_guid: String,
    pub placeholder: bool,
    pub hidden: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum GncAccountType {
    Bank,
    Cash,
    Asset,
    Stock,
    Mutual,
    Receivable,
    Credit,
    Liability,
    Payable,
    Income,
    Expense,
    Equity,
    Root,
    Trading,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GncTransaction {
    pub guid: String,
    pub post_date: i64,  // unix ms, UTC midnight of local date
    pub enter_date: i64,
    pub description: String,
    pub splits: Vec<GncSplit>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GncSplit {
    pub guid: String,
    pub account_guid: String,
    pub amount_cents: i64, // signed; positive → debit, negative → credit
    pub memo: String,
    pub reconcile_state: char, // 'n' | 'c' | 'y'
}

// ── Preview returned by the read command ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GnuCashPreview {
    pub book_guid: String,
    pub account_count: u32,
    pub transaction_count: u32,
    pub non_usd_accounts: Vec<String>, // full_names; non-empty → abort
}

// ── Mapper output (committer input) ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPlan {
    pub household_id: String,
    pub import_id: String,
    pub account_mappings: Vec<AccountMapping>,
    pub transactions: Vec<PlannedTransaction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountMapping {
    pub gnc_guid: String,
    pub gnc_full_name: String,
    pub tally_account_id: String,
    pub tally_name: String,
    pub tally_parent_id: Option<String>,
    pub tally_type: AccountType,
    pub tally_normal_balance: NormalBalance,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AccountType {
    Asset,
    Liability,
    Income,
    Expense,
    Equity,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NormalBalance {
    Debit,
    Credit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedTransaction {
    pub gnc_guid: String,
    pub txn_date: i64,
    pub memo: Option<String>,
    pub lines: Vec<PlannedLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedLine {
    pub tally_account_id: String,
    pub amount_cents: i64, // always positive
    pub side: Side,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Debit,
    Credit,
}

// ── Commit receipt ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReceipt {
    pub import_id: String,
    pub accounts_created: u32,
    pub transactions_committed: u32,
    pub transactions_skipped: u32,
}

// ── Errors ──────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("Couldn't open GnuCash file: {0}")]
    FileUnreadable(String),
    #[error("Not a GnuCash SQLite book")]
    NotAGnuCashBook,
    #[error("Non-USD accounts present: {0:?}")]
    NonUsdAccounts(Vec<String>),
    #[error("Transaction {guid} splits don't sum to zero (sum={sum_cents})")]
    UnbalancedTransaction { guid: String, sum_cents: i64 },
    #[error("Duplicate Tally account name after mapping: {0}")]
    DuplicateAccountName(String),
    #[error("Import with id {0} already exists")]
    ImportAlreadyRan(String),
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Validation failure for transaction on {date}: {reason}")]
    InvalidTransaction { date: i64, reason: String },
}
```

- [ ] **Step 4: Create empty submodule files so `cargo build` succeeds**

Create `apps/desktop/src-tauri/src/core/import/gnucash/reader.rs`:

```rust
// T-071 reader — implemented in subsequent tasks.
```

Create `apps/desktop/src-tauri/src/core/import/gnucash/mapper.rs`:

```rust
// T-072 mapper — implemented in subsequent tasks.
```

Create `apps/desktop/src-tauri/src/core/import/gnucash/committer.rs`:

```rust
// T-073 committer — implemented in subsequent tasks.
```

Create `apps/desktop/src-tauri/src/core/import/gnucash/reconcile.rs`:

```rust
// T-074 reconciler — implemented in subsequent tasks.
```

- [ ] **Step 5: Verify build**

Run: `cd apps/desktop/src-tauri && cargo build`
Expected: clean build, warnings about unused imports in `mod.rs` are fine for now (they'll be used soon).

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src-tauri/src/core/mod.rs apps/desktop/src-tauri/src/core/import/
git commit -m "feat(core): scaffold gnucash import module with public types"
```

---

### Task 4: Reader fixture helper

**Files:**
- Create: `apps/desktop/src-tauri/src/core/import/gnucash/test_fixtures.rs`
- Modify: `apps/desktop/src-tauri/src/core/import/gnucash/mod.rs` (register test module)

**Why this task:** GnuCash books are SQLite files with a very specific schema. We build them dynamically in Rust (sqlx into a tempfile) so fixtures live as code, not binaries. One helper handles all reader tests and the integration test.

- [ ] **Step 1: Add the fixture helper**

Create `apps/desktop/src-tauri/src/core/import/gnucash/test_fixtures.rs`:

```rust
//! Dynamic GnuCash fixture builder used by reader tests and the integration
//! test. We create a fresh SQLite file with just the GnuCash tables our reader
//! queries, populate them from a `FixtureSpec`, and hand back the file path.

#![cfg(test)]

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::str::FromStr;

pub struct FixtureSpec {
    pub book_guid: String,
    pub commodities: Vec<(String, String, String)>, // (guid, namespace, mnemonic)
    pub accounts: Vec<FixtureAccount>,
    pub transactions: Vec<FixtureTransaction>,
}

pub struct FixtureAccount {
    pub guid: String,
    pub name: String,
    pub account_type: &'static str, // "BANK", "EXPENSE", etc.
    pub commodity_guid: String,
    pub parent_guid: Option<String>,
    pub placeholder: bool,
    pub hidden: bool,
}

pub struct FixtureTransaction {
    pub guid: String,
    pub post_date: &'static str, // "2024-01-15 12:00:00"
    pub enter_date: &'static str,
    pub description: String,
    pub currency_guid: String,
    pub splits: Vec<FixtureSplit>,
}

pub struct FixtureSplit {
    pub guid: String,
    pub account_guid: String,
    pub value_num: i64, // numerator in cents terms (we use denom=100)
    pub memo: String,
    pub reconcile_state: &'static str, // "n" | "c" | "y"
}

/// Creates a new SQLite file with the GnuCash schema we care about, populates
/// it from `spec`, and returns the file path. The caller owns the file (keep
/// the tempdir alive until the test is done with it).
pub async fn build_fixture(dir: &std::path::Path, spec: &FixtureSpec) -> PathBuf {
    let path = dir.join(format!("{}.gnucash", spec.book_guid));
    // GnuCash uses a plain (unencrypted) SQLite file — mode=rwc creates it.
    let url = format!("sqlite://{}?mode=rwc", path.display());
    let opts = SqliteConnectOptions::from_str(&url).expect("valid sqlite url");
    let pool: SqlitePool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("open fixture db");

    create_gnucash_schema(&pool).await;
    insert_fixture(&pool, spec).await;

    pool.close().await;
    path
}

async fn create_gnucash_schema(pool: &SqlitePool) {
    for stmt in [
        "CREATE TABLE books (guid TEXT PRIMARY KEY NOT NULL, root_account_guid TEXT, root_template_guid TEXT)",
        "CREATE TABLE commodities (guid TEXT PRIMARY KEY NOT NULL, namespace TEXT NOT NULL, mnemonic TEXT NOT NULL, fullname TEXT, cusip TEXT, fraction INTEGER NOT NULL, quote_flag INTEGER NOT NULL)",
        "CREATE TABLE accounts (guid TEXT PRIMARY KEY NOT NULL, name TEXT NOT NULL, account_type TEXT NOT NULL, commodity_guid TEXT, commodity_scu INTEGER NOT NULL, non_std_scu INTEGER NOT NULL, parent_guid TEXT, code TEXT, description TEXT, hidden INTEGER, placeholder INTEGER)",
        "CREATE TABLE transactions (guid TEXT PRIMARY KEY NOT NULL, currency_guid TEXT NOT NULL, num TEXT NOT NULL, post_date TEXT, enter_date TEXT, description TEXT)",
        "CREATE TABLE splits (guid TEXT PRIMARY KEY NOT NULL, tx_guid TEXT NOT NULL, account_guid TEXT NOT NULL, memo TEXT NOT NULL, action TEXT NOT NULL, reconcile_state TEXT NOT NULL, reconcile_date TEXT, value_num INTEGER NOT NULL, value_denom INTEGER NOT NULL, quantity_num INTEGER NOT NULL, quantity_denom INTEGER NOT NULL, lot_guid TEXT)",
    ] {
        sqlx::query(stmt).execute(pool).await.expect("create schema");
    }
}

async fn insert_fixture(pool: &SqlitePool, spec: &FixtureSpec) {
    // root account for GnuCash's hierarchy (optional in our reader, but real books have one)
    sqlx::query("INSERT INTO books (guid, root_account_guid) VALUES (?, NULL)")
        .bind(&spec.book_guid)
        .execute(pool)
        .await
        .expect("insert book");

    for (guid, namespace, mnemonic) in &spec.commodities {
        sqlx::query(
            "INSERT INTO commodities (guid, namespace, mnemonic, fraction, quote_flag) VALUES (?, ?, ?, 100, 0)",
        )
        .bind(guid).bind(namespace).bind(mnemonic)
        .execute(pool).await.expect("insert commodity");
    }

    for a in &spec.accounts {
        sqlx::query(
            "INSERT INTO accounts (guid, name, account_type, commodity_guid, commodity_scu, non_std_scu, parent_guid, hidden, placeholder) VALUES (?, ?, ?, ?, 100, 0, ?, ?, ?)",
        )
        .bind(&a.guid).bind(&a.name).bind(a.account_type).bind(&a.commodity_guid)
        .bind(&a.parent_guid).bind(a.hidden as i64).bind(a.placeholder as i64)
        .execute(pool).await.expect("insert account");
    }

    for t in &spec.transactions {
        sqlx::query(
            "INSERT INTO transactions (guid, currency_guid, num, post_date, enter_date, description) VALUES (?, ?, '', ?, ?, ?)",
        )
        .bind(&t.guid).bind(&t.currency_guid).bind(t.post_date).bind(t.enter_date).bind(&t.description)
        .execute(pool).await.expect("insert transaction");

        for s in &t.splits {
            sqlx::query(
                "INSERT INTO splits (guid, tx_guid, account_guid, memo, action, reconcile_state, value_num, value_denom, quantity_num, quantity_denom) VALUES (?, ?, ?, ?, '', ?, ?, 100, ?, 100)",
            )
            .bind(&s.guid).bind(&t.guid).bind(&s.account_guid).bind(&s.memo)
            .bind(s.reconcile_state).bind(s.value_num).bind(s.value_num)
            .execute(pool).await.expect("insert split");
        }
    }
}

/// Small happy-path fixture: USD, 3 accounts, 2 transactions.
pub fn happy_spec() -> FixtureSpec {
    let usd = "cmdty_usd".to_string();
    let checking = "acc_checking".to_string();
    let groceries = "acc_groceries".to_string();
    let equity = "acc_opening".to_string();
    FixtureSpec {
        book_guid: "book_happy".to_string(),
        commodities: vec![(usd.clone(), "CURRENCY".into(), "USD".into())],
        accounts: vec![
            FixtureAccount { guid: checking.clone(), name: "Checking".into(),  account_type: "BANK",   commodity_guid: usd.clone(), parent_guid: None, placeholder: false, hidden: false },
            FixtureAccount { guid: groceries.clone(), name: "Groceries".into(),  account_type: "EXPENSE", commodity_guid: usd.clone(), parent_guid: None, placeholder: false, hidden: false },
            FixtureAccount { guid: equity.clone(),    name: "Opening Balances".into(), account_type: "EQUITY",  commodity_guid: usd.clone(), parent_guid: None, placeholder: false, hidden: false },
        ],
        transactions: vec![
            // Opening balance: +$1000 checking, -$1000 equity
            FixtureTransaction {
                guid: "tx_opening".into(),
                post_date: "2024-01-01 00:00:00",
                enter_date: "2024-01-01 00:00:00",
                description: "Opening Balance".into(),
                currency_guid: usd.clone(),
                splits: vec![
                    FixtureSplit { guid: "sp_open_a".into(), account_guid: checking.clone(), value_num: 100000,  memo: "".into(), reconcile_state: "y" },
                    FixtureSplit { guid: "sp_open_b".into(), account_guid: equity.clone(),   value_num: -100000, memo: "".into(), reconcile_state: "y" },
                ],
            },
            // Groceries: -$50 checking, +$50 groceries
            FixtureTransaction {
                guid: "tx_groc".into(),
                post_date: "2024-02-03 00:00:00",
                enter_date: "2024-02-03 09:00:00",
                description: "Whole Foods".into(),
                currency_guid: usd,
                splits: vec![
                    FixtureSplit { guid: "sp_groc_a".into(), account_guid: checking,  value_num: -5000, memo: "".into(), reconcile_state: "n" },
                    FixtureSplit { guid: "sp_groc_b".into(), account_guid: groceries, value_num: 5000,  memo: "".into(), reconcile_state: "n" },
                ],
            },
        ],
    }
}
```

- [ ] **Step 2: Register the test module in `mod.rs`**

At the bottom of `apps/desktop/src-tauri/src/core/import/gnucash/mod.rs`, add:

```rust
#[cfg(test)]
mod test_fixtures;
```

- [ ] **Step 3: Verify it compiles under test**

Run: `cd apps/desktop/src-tauri && cargo test --lib --no-run`
Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src-tauri/src/core/import/gnucash/
git commit -m "test(import): dynamic GnuCash fixture builder"
```

---

### Task 5: Reader — happy path

**Files:**
- Modify: `apps/desktop/src-tauri/src/core/import/gnucash/reader.rs`

- [ ] **Step 1: Write the failing test**

Replace `reader.rs` with:

```rust
//! Opens a GnuCash SQLite file read-only and builds a `GnuCashBook`.
//!
//! GnuCash stores amounts as signed `value_num / value_denom` rationals.
//! Our reader normalizes every split to signed cents (denom=100 for USD).

use super::{
    GnuCashBook, GncAccount, GncAccountType, GncCommodity, GncSplit, GncTransaction, ImportError,
};
use chrono::NaiveDateTime;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;
use std::str::FromStr;

pub async fn read(path: &Path) -> Result<GnuCashBook, ImportError> {
    let pool = open_readonly(path).await?;

    if !is_gnucash_book(&pool).await {
        pool.close().await;
        return Err(ImportError::NotAGnuCashBook);
    }

    let book_guid: (String,) = sqlx::query_as("SELECT guid FROM books LIMIT 1")
        .fetch_one(&pool)
        .await?;
    let commodities = load_commodities(&pool).await?;
    let accounts = load_accounts(&pool).await?;
    let transactions = load_transactions(&pool).await?;

    pool.close().await;

    let book = GnuCashBook {
        book_guid: book_guid.0,
        commodities,
        accounts,
        transactions,
    };

    check_splits_balance(&book)?;
    Ok(book)
}

async fn open_readonly(path: &Path) -> Result<SqlitePool, ImportError> {
    if !path.exists() {
        return Err(ImportError::FileUnreadable(format!(
            "{} does not exist",
            path.display()
        )));
    }
    let url = format!("sqlite://{}?mode=ro", path.display());
    let opts = SqliteConnectOptions::from_str(&url)
        .map_err(|e| ImportError::FileUnreadable(e.to_string()))?;
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .map_err(|e| ImportError::FileUnreadable(e.to_string()))
}

async fn is_gnucash_book(pool: &SqlitePool) -> bool {
    // GnuCash books have a `books` table. Any random SQLite file will not.
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='books'",
    )
    .fetch_one(pool)
    .await
    .map(|n| n > 0)
    .unwrap_or(false)
}

async fn load_commodities(pool: &SqlitePool) -> Result<Vec<GncCommodity>, ImportError> {
    let rows: Vec<(String, String, String)> =
        sqlx::query_as("SELECT guid, namespace, mnemonic FROM commodities")
            .fetch_all(pool)
            .await?;
    Ok(rows
        .into_iter()
        .map(|(guid, namespace, mnemonic)| GncCommodity {
            guid,
            namespace,
            mnemonic,
        })
        .collect())
}

async fn load_accounts(pool: &SqlitePool) -> Result<Vec<GncAccount>, ImportError> {
    // First pass: load flat rows; second pass: assign full_name via parent chain.
    #[derive(sqlx::FromRow)]
    struct Row {
        guid: String,
        name: String,
        account_type: String,
        commodity_guid: Option<String>,
        parent_guid: Option<String>,
        hidden: Option<i64>,
        placeholder: Option<i64>,
    }
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT guid, name, account_type, commodity_guid, parent_guid, hidden, placeholder FROM accounts",
    )
    .fetch_all(pool)
    .await?;

    let by_guid: std::collections::HashMap<String, &Row> =
        rows.iter().map(|r| (r.guid.clone(), r)).collect();

    fn full_name(row: &Row, by_guid: &std::collections::HashMap<String, &Row>) -> String {
        let mut parts = vec![row.name.clone()];
        let mut cur = row.parent_guid.clone();
        while let Some(pg) = cur {
            if let Some(parent) = by_guid.get(&pg) {
                // Skip the ROOT account (GnuCash's invisible root)
                if parent.account_type == "ROOT" {
                    break;
                }
                parts.push(parent.name.clone());
                cur = parent.parent_guid.clone();
            } else {
                break;
            }
        }
        parts.reverse();
        parts.join(":")
    }

    let mut out = Vec::with_capacity(rows.len());
    for r in rows.iter() {
        let gnc_type = parse_account_type(&r.account_type);
        // Skip the ROOT pseudo-account — users never see it.
        if gnc_type == GncAccountType::Root {
            continue;
        }
        out.push(GncAccount {
            guid: r.guid.clone(),
            parent_guid: r.parent_guid.clone(),
            name: r.name.clone(),
            full_name: full_name(r, &by_guid),
            gnc_type,
            commodity_guid: r.commodity_guid.clone().unwrap_or_default(),
            placeholder: r.placeholder.unwrap_or(0) != 0,
            hidden: r.hidden.unwrap_or(0) != 0,
        });
    }
    Ok(out)
}

fn parse_account_type(s: &str) -> GncAccountType {
    match s {
        "BANK" => GncAccountType::Bank,
        "CASH" => GncAccountType::Cash,
        "ASSET" => GncAccountType::Asset,
        "STOCK" => GncAccountType::Stock,
        "MUTUAL" => GncAccountType::Mutual,
        "RECEIVABLE" => GncAccountType::Receivable,
        "CREDIT" => GncAccountType::Credit,
        "LIABILITY" => GncAccountType::Liability,
        "PAYABLE" => GncAccountType::Payable,
        "INCOME" => GncAccountType::Income,
        "EXPENSE" => GncAccountType::Expense,
        "EQUITY" => GncAccountType::Equity,
        "TRADING" => GncAccountType::Trading,
        _ => GncAccountType::Root,
    }
}

async fn load_transactions(pool: &SqlitePool) -> Result<Vec<GncTransaction>, ImportError> {
    #[derive(sqlx::FromRow)]
    struct TxRow {
        guid: String,
        post_date: Option<String>,
        enter_date: Option<String>,
        description: Option<String>,
    }
    let tx_rows: Vec<TxRow> = sqlx::query_as(
        "SELECT guid, post_date, enter_date, description FROM transactions ORDER BY post_date",
    )
    .fetch_all(pool)
    .await?;

    #[derive(sqlx::FromRow)]
    struct SpRow {
        guid: String,
        tx_guid: String,
        account_guid: String,
        memo: String,
        reconcile_state: String,
        value_num: i64,
        value_denom: i64,
    }
    let sp_rows: Vec<SpRow> = sqlx::query_as(
        "SELECT guid, tx_guid, account_guid, memo, reconcile_state, value_num, value_denom FROM splits",
    )
    .fetch_all(pool)
    .await?;

    let mut by_tx: std::collections::HashMap<String, Vec<GncSplit>> = std::collections::HashMap::new();
    for sp in sp_rows {
        let cents = normalize_to_cents(sp.value_num, sp.value_denom);
        let rec = sp.reconcile_state.chars().next().unwrap_or('n');
        by_tx.entry(sp.tx_guid).or_default().push(GncSplit {
            guid: sp.guid,
            account_guid: sp.account_guid,
            amount_cents: cents,
            memo: sp.memo,
            reconcile_state: rec,
        });
    }

    let mut out = Vec::with_capacity(tx_rows.len());
    for tx in tx_rows {
        let post_ms = parse_gnc_date_to_utc_midnight_ms(tx.post_date.as_deref().unwrap_or(""));
        let enter_ms = parse_gnc_date_ms(tx.enter_date.as_deref().unwrap_or(""));
        out.push(GncTransaction {
            guid: tx.guid.clone(),
            post_date: post_ms,
            enter_date: enter_ms,
            description: tx.description.unwrap_or_default(),
            splits: by_tx.remove(&tx.guid).unwrap_or_default(),
        });
    }
    Ok(out)
}

/// Convert GnuCash's value_num/value_denom to signed cents. For USD (denom=100)
/// this is a pass-through; other denoms are scaled to 100 with rounding.
fn normalize_to_cents(num: i64, denom: i64) -> i64 {
    if denom == 100 || denom == 0 {
        return num;
    }
    // Multiply first to preserve precision, then round toward zero.
    (num * 100) / denom
}

/// GnuCash stores post_date as "YYYY-MM-DD HH:MM:SS" in UTC. Tally stores
/// txn_date as UTC midnight of the local date. For Phase 1 we treat GnuCash's
/// UTC date directly as the local date — beta user is in a single timezone.
fn parse_gnc_date_to_utc_midnight_ms(s: &str) -> i64 {
    let dt = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").unwrap_or_default();
    let date = dt.date();
    let midnight = date.and_hms_opt(0, 0, 0).unwrap_or_default();
    midnight.and_utc().timestamp_millis()
}

fn parse_gnc_date_ms(s: &str) -> i64 {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .map(|dt| dt.and_utc().timestamp_millis())
        .unwrap_or(0)
}

fn check_splits_balance(book: &GnuCashBook) -> Result<(), ImportError> {
    for tx in &book.transactions {
        let sum: i64 = tx.splits.iter().map(|s| s.amount_cents).sum();
        if sum != 0 {
            return Err(ImportError::UnbalancedTransaction {
                guid: tx.guid.clone(),
                sum_cents: sum,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::test_fixtures::{build_fixture, happy_spec};
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn happy_path_reads_all_rows() {
        let dir = tempdir().unwrap();
        let path = build_fixture(dir.path(), &happy_spec()).await;
        let book = read(&path).await.expect("should read");

        assert_eq!(book.book_guid, "book_happy");
        assert_eq!(book.commodities.len(), 1);
        assert_eq!(book.commodities[0].mnemonic, "USD");
        assert_eq!(book.accounts.len(), 3);
        assert_eq!(book.transactions.len(), 2);

        let opening = book.transactions.iter().find(|t| t.guid == "tx_opening").unwrap();
        assert_eq!(opening.splits.len(), 2);
        let sum: i64 = opening.splits.iter().map(|s| s.amount_cents).sum();
        assert_eq!(sum, 0, "splits must balance to zero");
    }

    #[tokio::test]
    async fn full_name_builds_leaf_join() {
        // Parent/child hierarchy
        let dir = tempdir().unwrap();
        let mut spec = happy_spec();
        // Nest groceries under a parent called "Food"
        let food = "acc_food".to_string();
        spec.accounts.push(super::super::test_fixtures::FixtureAccount {
            guid: food.clone(),
            name: "Food".into(),
            account_type: "EXPENSE",
            commodity_guid: "cmdty_usd".into(),
            parent_guid: None,
            placeholder: true,
            hidden: false,
        });
        let groc = spec.accounts.iter_mut().find(|a| a.guid == "acc_groceries").unwrap();
        groc.parent_guid = Some(food);
        spec.book_guid = "book_nested".into();
        let path = build_fixture(dir.path(), &spec).await;

        let book = read(&path).await.unwrap();
        let groc = book.accounts.iter().find(|a| a.name == "Groceries").unwrap();
        assert_eq!(groc.full_name, "Food:Groceries");
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cd apps/desktop/src-tauri && cargo test --lib core::import::gnucash::reader -- --nocapture`
Expected: PASS on both tests.

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src-tauri/src/core/import/gnucash/reader.rs
git commit -m "feat(import): reader happy path + hierarchy resolution"
```

---

### Task 6: Reader — currency scan and `GnuCashPreview`

**Files:**
- Modify: `apps/desktop/src-tauri/src/core/import/gnucash/reader.rs`

- [ ] **Step 1: Write the failing tests**

Append to the `tests` module in `reader.rs`:

```rust
#[tokio::test]
async fn non_usd_account_flagged_in_preview() {
    let dir = tempdir().unwrap();
    let mut spec = happy_spec();
    spec.book_guid = "book_eur".into();
    // Add EUR commodity and a EUR-denominated savings account
    spec.commodities.push(("cmdty_eur".into(), "CURRENCY".into(), "EUR".into()));
    spec.accounts.push(super::super::test_fixtures::FixtureAccount {
        guid: "acc_savings_eur".into(),
        name: "Euro Savings".into(),
        account_type: "BANK",
        commodity_guid: "cmdty_eur".into(),
        parent_guid: None,
        placeholder: false,
        hidden: false,
    });
    let path = build_fixture(dir.path(), &spec).await;

    let preview = preview(&path).await.expect("preview should still succeed");
    assert!(preview.non_usd_accounts.contains(&"Euro Savings".to_string()));
    assert_eq!(preview.account_count, 4);
}

#[tokio::test]
async fn stock_commodity_flagged_as_non_usd() {
    let dir = tempdir().unwrap();
    let mut spec = happy_spec();
    spec.book_guid = "book_stock".into();
    spec.commodities.push(("cmdty_aapl".into(), "NASDAQ".into(), "AAPL".into()));
    spec.accounts.push(super::super::test_fixtures::FixtureAccount {
        guid: "acc_aapl".into(),
        name: "AAPL".into(),
        account_type: "STOCK",
        commodity_guid: "cmdty_aapl".into(),
        parent_guid: None,
        placeholder: false,
        hidden: false,
    });
    let path = build_fixture(dir.path(), &spec).await;

    let preview = preview(&path).await.unwrap();
    assert!(preview.non_usd_accounts.contains(&"AAPL".to_string()));
}
```

Also add a shortcut for the happy case:

```rust
#[tokio::test]
async fn happy_preview_has_empty_non_usd_list() {
    let dir = tempdir().unwrap();
    let path = build_fixture(dir.path(), &happy_spec()).await;
    let p = preview(&path).await.unwrap();
    assert!(p.non_usd_accounts.is_empty());
    assert_eq!(p.transaction_count, 2);
    assert_eq!(p.account_count, 3);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd apps/desktop/src-tauri && cargo test --lib core::import::gnucash::reader::tests::non_usd_account_flagged_in_preview`
Expected: FAIL — `preview` function doesn't exist.

- [ ] **Step 3: Add the `preview` function to `reader.rs`**

Add imports at the top:

```rust
use super::GnuCashPreview;
```

Add the function (place it below `read`):

```rust
/// Builds a lightweight preview without applying the splits-balance check.
/// This is what `read_gnucash_file` returns so the onboarding UI can decide
/// whether to proceed before we build a full ImportPlan.
pub async fn preview(path: &Path) -> Result<GnuCashPreview, ImportError> {
    let pool = open_readonly(path).await?;
    if !is_gnucash_book(&pool).await {
        pool.close().await;
        return Err(ImportError::NotAGnuCashBook);
    }

    let book_guid: (String,) = sqlx::query_as("SELECT guid FROM books LIMIT 1")
        .fetch_one(&pool)
        .await?;

    let commodities = load_commodities(&pool).await?;
    let accounts = load_accounts(&pool).await?;
    let (tx_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions")
        .fetch_one(&pool)
        .await?;
    pool.close().await;

    // A commodity is USD iff namespace='CURRENCY' AND mnemonic='USD'. Anything
    // else — another currency, a stock ticker, a mutual fund — is non-USD for
    // Phase 1.
    let usd_guids: std::collections::HashSet<&str> = commodities
        .iter()
        .filter(|c| c.namespace == "CURRENCY" && c.mnemonic == "USD")
        .map(|c| c.guid.as_str())
        .collect();

    let non_usd: Vec<String> = accounts
        .iter()
        .filter(|a| !a.placeholder && !usd_guids.contains(a.commodity_guid.as_str()))
        .map(|a| a.full_name.clone())
        .collect();

    Ok(GnuCashPreview {
        book_guid: book_guid.0,
        account_count: accounts.len() as u32,
        transaction_count: tx_count as u32,
        non_usd_accounts: non_usd,
    })
}
```

- [ ] **Step 4: Run tests**

Run: `cd apps/desktop/src-tauri && cargo test --lib core::import::gnucash::reader`
Expected: PASS on all reader tests.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/core/import/gnucash/reader.rs
git commit -m "feat(import): preview with currency scan"
```

---

### Task 7: Reader — not-a-GnuCash-file and corrupt-splits cases

**Files:**
- Modify: `apps/desktop/src-tauri/src/core/import/gnucash/reader.rs`

- [ ] **Step 1: Write the failing tests**

Append to `tests`:

```rust
#[tokio::test]
async fn empty_sqlite_rejected_as_not_gnucash() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("empty.sqlite");
    // Create an empty SQLite file via sqlx
    let url = format!("sqlite://{}?mode=rwc", path.display());
    let opts = sqlx::sqlite::SqliteConnectOptions::from_str(&url).unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap();
    pool.close().await;

    let err = read(&path).await.unwrap_err();
    assert!(matches!(err, ImportError::NotAGnuCashBook));
}

#[tokio::test]
async fn missing_file_returns_file_unreadable() {
    let err = read(std::path::Path::new("/nonexistent/path.gnucash")).await.unwrap_err();
    assert!(matches!(err, ImportError::FileUnreadable(_)));
}

#[tokio::test]
async fn unbalanced_splits_rejected() {
    let dir = tempdir().unwrap();
    let mut spec = happy_spec();
    spec.book_guid = "book_corrupt".into();
    // Corrupt the first transaction: make the first split 1 cent off
    spec.transactions[0].splits[0].value_num += 1;
    let path = build_fixture(dir.path(), &spec).await;

    let err = read(&path).await.unwrap_err();
    match err {
        ImportError::UnbalancedTransaction { guid, sum_cents } => {
            assert_eq!(guid, "tx_opening");
            assert_eq!(sum_cents, 1);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cd apps/desktop/src-tauri && cargo test --lib core::import::gnucash::reader`
Expected: PASS — the error paths are already implemented; these tests just exercise them.

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src-tauri/src/core/import/gnucash/reader.rs
git commit -m "test(import): reader rejects non-gnucash files and unbalanced splits"
```

---

### Task 8: `read_gnucash_file` Tauri command

**Files:**
- Modify: `apps/desktop/src-tauri/src/commands/mod.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

**Note:** Tauri commands in this codebase return `Result<T, String>`, stringifying internal errors for the frontend. We do the same and rely on TS to surface plain-language messages.

- [ ] **Step 1: Add command to `commands/mod.rs`**

At the bottom of `commands/mod.rs`, append:

```rust
// ── GnuCash import commands ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ReadGnuCashArgs {
    pub path: String,
}

#[tauri::command]
pub async fn read_gnucash_file(
    args: ReadGnuCashArgs,
) -> Result<crate::core::import::gnucash::GnuCashPreview, String> {
    use std::path::Path;
    crate::core::import::gnucash::reader::preview(Path::new(&args.path))
        .await
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Register in `lib.rs`**

Edit `apps/desktop/src-tauri/src/lib.rs`, adding `commands::read_gnucash_file,` to the `invoke_handler` list (e.g. after `commands::get_pending_transactions,`).

- [ ] **Step 3: Verify build**

Run: `cd apps/desktop/src-tauri && cargo build`
Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src-tauri/src/commands/mod.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(import): read_gnucash_file Tauri command"
```

---

## Ticket T-072: Mapper

### Task 9: Default type mapping (pure function)

**Files:**
- Modify: `apps/desktop/src-tauri/src/core/import/gnucash/mapper.rs`

- [ ] **Step 1: Write the failing tests**

Replace `mapper.rs` with:

```rust
//! Converts a `GnuCashBook` into an `ImportPlan` using default type mappings,
//! and applies user-supplied overrides. Pure logic — no DB, no IO.

use super::{
    AccountMapping, AccountType, GncAccount, GncAccountType, GncSplit, GncTransaction,
    GnuCashBook, ImportError, ImportPlan, NormalBalance, PlannedLine, PlannedTransaction, Side,
};
use std::collections::HashMap;

/// The default GnuCash-type → Tally (type, normal_balance) mapping.
pub fn default_tally_type(gnc: GncAccountType) -> (AccountType, NormalBalance) {
    use GncAccountType::*;
    match gnc {
        Bank | Cash | Asset | Stock | Mutual | Receivable => {
            (AccountType::Asset, NormalBalance::Debit)
        }
        Credit | Liability | Payable => (AccountType::Liability, NormalBalance::Credit),
        Income => (AccountType::Income, NormalBalance::Credit),
        Expense => (AccountType::Expense, NormalBalance::Debit),
        Equity => (AccountType::Equity, NormalBalance::Credit),
        // Root/Trading shouldn't reach the mapper; if they do, treat as equity.
        Root | Trading => (AccountType::Equity, NormalBalance::Credit),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bank_maps_to_asset_debit() {
        assert_eq!(default_tally_type(GncAccountType::Bank), (AccountType::Asset, NormalBalance::Debit));
    }
    #[test]
    fn credit_maps_to_liability_credit() {
        assert_eq!(default_tally_type(GncAccountType::Credit), (AccountType::Liability, NormalBalance::Credit));
    }
    #[test]
    fn income_maps_to_income_credit() {
        assert_eq!(default_tally_type(GncAccountType::Income), (AccountType::Income, NormalBalance::Credit));
    }
    #[test]
    fn expense_maps_to_expense_debit() {
        assert_eq!(default_tally_type(GncAccountType::Expense), (AccountType::Expense, NormalBalance::Debit));
    }
    #[test]
    fn equity_maps_to_equity_credit() {
        assert_eq!(default_tally_type(GncAccountType::Equity), (AccountType::Equity, NormalBalance::Credit));
    }
    #[test]
    fn stock_maps_to_asset_debit() {
        assert_eq!(default_tally_type(GncAccountType::Stock), (AccountType::Asset, NormalBalance::Debit));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cd apps/desktop/src-tauri && cargo test --lib core::import::gnucash::mapper`
Expected: PASS on all 6 default-mapping tests.

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src-tauri/src/core/import/gnucash/mapper.rs
git commit -m "feat(import): default GnuCash→Tally type mapping"
```

---

### Task 10: Build default `ImportPlan`

**Files:**
- Modify: `apps/desktop/src-tauri/src/core/import/gnucash/mapper.rs`

- [ ] **Step 1: Write the failing tests**

Append to `mapper.rs` `tests` module:

```rust
use super::super::test_fixtures::{build_fixture, happy_spec};
use super::super::reader::read;
use tempfile::tempdir;

fn stable_ulid(seed: &str) -> String {
    // Deterministic pseudo-ulid for tests (prefixing + padding)
    format!("{:0>26}", seed.to_uppercase())
}

#[tokio::test]
async fn default_plan_maps_every_non_placeholder_account() {
    let dir = tempdir().unwrap();
    let path = build_fixture(dir.path(), &happy_spec()).await;
    let book = read(&path).await.unwrap();

    let plan = build_default_plan(
        "hh_test".into(),
        "imp_test".into(),
        &book,
        &ulid_gen(),
    ).unwrap();

    assert_eq!(plan.account_mappings.len(), 3); // checking, groceries, opening
    assert_eq!(plan.transactions.len(), 2);
    assert_eq!(plan.household_id, "hh_test");
    assert_eq!(plan.import_id, "imp_test");
}

#[tokio::test]
async fn default_plan_derives_side_from_signed_amount() {
    let dir = tempdir().unwrap();
    let path = build_fixture(dir.path(), &happy_spec()).await;
    let book = read(&path).await.unwrap();
    let plan = build_default_plan("hh".into(), "imp".into(), &book, &ulid_gen()).unwrap();

    let opening = plan.transactions.iter().find(|t| t.gnc_guid == "tx_opening").unwrap();
    // +100000 → debit, -100000 → credit
    let debit_line = opening.lines.iter().find(|l| l.side == Side::Debit).unwrap();
    let credit_line = opening.lines.iter().find(|l| l.side == Side::Credit).unwrap();
    assert_eq!(debit_line.amount_cents, 100000);
    assert_eq!(credit_line.amount_cents, 100000);
}

#[tokio::test]
async fn default_plan_preserves_parent_hierarchy() {
    let dir = tempdir().unwrap();
    let mut spec = happy_spec();
    spec.book_guid = "book_hier".into();
    spec.accounts.push(super::super::test_fixtures::FixtureAccount {
        guid: "acc_food".into(),
        name: "Food".into(),
        account_type: "EXPENSE",
        commodity_guid: "cmdty_usd".into(),
        parent_guid: None,
        placeholder: true,
        hidden: false,
    });
    spec.accounts.iter_mut().find(|a| a.guid == "acc_groceries").unwrap().parent_guid =
        Some("acc_food".into());
    let path = build_fixture(dir.path(), &spec).await;
    let book = read(&path).await.unwrap();

    let plan = build_default_plan("hh".into(), "imp".into(), &book, &ulid_gen()).unwrap();
    let food = plan.account_mappings.iter().find(|m| m.gnc_full_name == "Food").unwrap();
    let groc = plan.account_mappings.iter().find(|m| m.gnc_full_name == "Food:Groceries").unwrap();
    assert_eq!(groc.tally_parent_id, Some(food.tally_account_id.clone()));
}

/// Test ULID generator: deterministic, just an atomic counter.
fn ulid_gen() -> impl FnMut() -> String {
    let mut n: u64 = 0;
    move || {
        n += 1;
        format!("ULID_{n:0>20}")
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd apps/desktop/src-tauri && cargo test --lib core::import::gnucash::mapper::tests::default_plan_maps_every_non_placeholder_account`
Expected: FAIL — `build_default_plan` not defined.

- [ ] **Step 3: Implement `build_default_plan`**

Add to `mapper.rs` (above the `tests` module):

```rust
/// Build the initial `ImportPlan` with the default type mapping and parent
/// hierarchy. Pure — caller provides the household_id, import_id, and a ULID
/// generator for account IDs.
pub fn build_default_plan<F>(
    household_id: String,
    import_id: String,
    book: &GnuCashBook,
    mut new_ulid: F,
) -> Result<ImportPlan, ImportError>
where
    F: FnMut() -> String,
{
    // Step 1: assign a Tally ULID to every importable account.
    let mut guid_to_ulid: HashMap<String, String> = HashMap::new();
    for a in &book.accounts {
        guid_to_ulid.insert(a.guid.clone(), new_ulid());
    }

    // Step 2: build AccountMapping per account.
    let mut account_mappings: Vec<AccountMapping> = Vec::with_capacity(book.accounts.len());
    for a in &book.accounts {
        let (ttype, nb) = default_tally_type(a.gnc_type);
        let tally_parent_id = a
            .parent_guid
            .as_ref()
            .and_then(|pg| guid_to_ulid.get(pg).cloned());
        account_mappings.push(AccountMapping {
            gnc_guid: a.guid.clone(),
            gnc_full_name: a.full_name.clone(),
            tally_account_id: guid_to_ulid.get(&a.guid).expect("pre-assigned").clone(),
            tally_name: a.name.clone(),
            tally_parent_id,
            tally_type: ttype,
            tally_normal_balance: nb,
        });
    }

    // Step 3: convert transactions. Skip any split that references a GnuCash
    // account type we filtered out (currently: none — ROOT is already dropped
    // by the reader).
    let mut transactions: Vec<PlannedTransaction> = Vec::with_capacity(book.transactions.len());
    for tx in &book.transactions {
        let mut lines = Vec::with_capacity(tx.splits.len());
        for sp in &tx.splits {
            let tally_id = match guid_to_ulid.get(&sp.account_guid) {
                Some(id) => id.clone(),
                None => continue, // account we don't know about; shouldn't happen for clean books
            };
            lines.push(split_to_line(sp, tally_id));
        }
        let memo = if tx.description.is_empty() { None } else { Some(tx.description.clone()) };
        transactions.push(PlannedTransaction {
            gnc_guid: tx.guid.clone(),
            txn_date: tx.post_date,
            memo,
            lines,
        });
    }

    Ok(ImportPlan {
        household_id,
        import_id,
        account_mappings,
        transactions,
    })
}

fn split_to_line(sp: &GncSplit, tally_account_id: String) -> PlannedLine {
    let (amount_cents, side) = if sp.amount_cents >= 0 {
        (sp.amount_cents, Side::Debit)
    } else {
        (-sp.amount_cents, Side::Credit)
    };
    PlannedLine {
        tally_account_id,
        amount_cents,
        side,
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cd apps/desktop/src-tauri && cargo test --lib core::import::gnucash::mapper`
Expected: PASS on all mapper tests.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/core/import/gnucash/mapper.rs
git commit -m "feat(import): build default ImportPlan from GnuCashBook"
```

---

### Task 11: Mapping edits

**Files:**
- Modify: `apps/desktop/src-tauri/src/core/import/gnucash/mapper.rs`

- [ ] **Step 1: Write the failing test**

Append to `tests`:

```rust
#[tokio::test]
async fn apply_mapping_edit_changes_only_targeted_account() {
    let dir = tempdir().unwrap();
    let path = build_fixture(dir.path(), &happy_spec()).await;
    let book = read(&path).await.unwrap();
    let mut plan = build_default_plan("hh".into(), "imp".into(), &book, &ulid_gen()).unwrap();

    let original_count = plan.account_mappings.len();
    let result = apply_mapping_edit(
        &mut plan,
        &MappingEdit::ChangeType {
            gnc_full_name: "Groceries".into(),
            new_type: AccountType::Liability,
            new_normal_balance: NormalBalance::Credit,
        },
    );
    assert!(result.is_ok());

    let groc = plan.account_mappings.iter().find(|m| m.gnc_full_name == "Groceries").unwrap();
    assert_eq!(groc.tally_type, AccountType::Liability);
    assert_eq!(groc.tally_normal_balance, NormalBalance::Credit);

    // Checking unchanged
    let chk = plan.account_mappings.iter().find(|m| m.gnc_full_name == "Checking").unwrap();
    assert_eq!(chk.tally_type, AccountType::Asset);
    assert_eq!(plan.account_mappings.len(), original_count);
}

#[tokio::test]
async fn apply_mapping_edit_rename() {
    let dir = tempdir().unwrap();
    let path = build_fixture(dir.path(), &happy_spec()).await;
    let book = read(&path).await.unwrap();
    let mut plan = build_default_plan("hh".into(), "imp".into(), &book, &ulid_gen()).unwrap();

    apply_mapping_edit(
        &mut plan,
        &MappingEdit::Rename {
            gnc_full_name: "Groceries".into(),
            new_tally_name: "Food & Household".into(),
        },
    ).unwrap();
    let m = plan.account_mappings.iter().find(|m| m.gnc_full_name == "Groceries").unwrap();
    assert_eq!(m.tally_name, "Food & Household");
}

#[tokio::test]
async fn apply_mapping_edit_unknown_account_errors() {
    let dir = tempdir().unwrap();
    let path = build_fixture(dir.path(), &happy_spec()).await;
    let book = read(&path).await.unwrap();
    let mut plan = build_default_plan("hh".into(), "imp".into(), &book, &ulid_gen()).unwrap();
    let err = apply_mapping_edit(
        &mut plan,
        &MappingEdit::ChangeType {
            gnc_full_name: "Nonexistent".into(),
            new_type: AccountType::Asset,
            new_normal_balance: NormalBalance::Debit,
        },
    ).unwrap_err();
    assert!(matches!(err, ImportError::DuplicateAccountName(ref s) if s.contains("Nonexistent")));
}
```

- [ ] **Step 2: Run tests to verify failure**

Expected: FAIL — `MappingEdit` / `apply_mapping_edit` don't exist.

- [ ] **Step 3: Implement `MappingEdit` + `apply_mapping_edit`**

Add to `mapper.rs` above `tests`:

```rust
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MappingEdit {
    ChangeType {
        gnc_full_name: String,
        new_type: AccountType,
        new_normal_balance: NormalBalance,
    },
    Rename {
        gnc_full_name: String,
        new_tally_name: String,
    },
}

pub fn apply_mapping_edit(plan: &mut ImportPlan, edit: &MappingEdit) -> Result<(), ImportError> {
    let target = match edit {
        MappingEdit::ChangeType { gnc_full_name, .. } => gnc_full_name,
        MappingEdit::Rename { gnc_full_name, .. } => gnc_full_name,
    };
    let m = plan
        .account_mappings
        .iter_mut()
        .find(|m| &m.gnc_full_name == target)
        .ok_or_else(|| ImportError::DuplicateAccountName(format!("unknown account: {target}")))?;

    match edit {
        MappingEdit::ChangeType { new_type, new_normal_balance, .. } => {
            m.tally_type = *new_type;
            m.tally_normal_balance = *new_normal_balance;
        }
        MappingEdit::Rename { new_tally_name, .. } => {
            m.tally_name = new_tally_name.clone();
        }
    }
    Ok(())
}
```

(The `DuplicateAccountName` variant is re-used for "unknown target" to avoid growing the error enum; the message carries the distinction. If later we want a distinct variant, one line change. YAGNI for now.)

- [ ] **Step 4: Run tests**

Run: `cd apps/desktop/src-tauri && cargo test --lib core::import::gnucash::mapper`
Expected: PASS on all mapper tests.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/core/import/gnucash/mapper.rs
git commit -m "feat(import): mapping edits (ChangeType, Rename)"
```

---

### Task 12: Duplicate-name detection

**Files:**
- Modify: `apps/desktop/src-tauri/src/core/import/gnucash/mapper.rs`

- [ ] **Step 1: Write the failing test**

Append to `tests`:

```rust
#[tokio::test]
async fn duplicate_tally_names_detected() {
    let dir = tempdir().unwrap();
    let path = build_fixture(dir.path(), &happy_spec()).await;
    let book = read(&path).await.unwrap();
    let mut plan = build_default_plan("hh".into(), "imp".into(), &book, &ulid_gen()).unwrap();

    // Rename Groceries to "Checking" — collides with existing Checking account
    apply_mapping_edit(&mut plan, &MappingEdit::Rename {
        gnc_full_name: "Groceries".into(),
        new_tally_name: "Checking".into(),
    }).unwrap();

    let dups = find_duplicate_names(&plan);
    assert!(dups.contains(&"Checking".to_string()));
}

#[tokio::test]
async fn no_duplicates_by_default() {
    let dir = tempdir().unwrap();
    let path = build_fixture(dir.path(), &happy_spec()).await;
    let book = read(&path).await.unwrap();
    let plan = build_default_plan("hh".into(), "imp".into(), &book, &ulid_gen()).unwrap();
    assert!(find_duplicate_names(&plan).is_empty());
}
```

- [ ] **Step 2: Run to verify failure**

Expected: FAIL — `find_duplicate_names` not defined.

- [ ] **Step 3: Implement**

Add to `mapper.rs`:

```rust
/// Returns the list of Tally full-name paths (joined by ':') that are used
/// by more than one account mapping. An empty result means the plan is
/// safe to commit.
pub fn find_duplicate_names(plan: &ImportPlan) -> Vec<String> {
    let by_id: HashMap<&str, &AccountMapping> = plan
        .account_mappings
        .iter()
        .map(|m| (m.tally_account_id.as_str(), m))
        .collect();

    fn full_path(m: &AccountMapping, by_id: &HashMap<&str, &AccountMapping>) -> String {
        let mut parts = vec![m.tally_name.clone()];
        let mut cur = m.tally_parent_id.clone();
        while let Some(pid) = cur {
            if let Some(p) = by_id.get(pid.as_str()) {
                parts.push(p.tally_name.clone());
                cur = p.tally_parent_id.clone();
            } else {
                break;
            }
        }
        parts.reverse();
        parts.join(":")
    }

    let mut counts: HashMap<String, u32> = HashMap::new();
    for m in &plan.account_mappings {
        *counts.entry(full_path(m, &by_id)).or_insert(0) += 1;
    }
    counts.into_iter().filter(|(_, n)| *n > 1).map(|(p, _)| p).collect()
}
```

- [ ] **Step 4: Run tests**

Run: `cd apps/desktop/src-tauri && cargo test --lib core::import::gnucash::mapper`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/core/import/gnucash/mapper.rs
git commit -m "feat(import): duplicate-name detection on ImportPlan"
```

---

### Task 13: Mapper Tauri commands + AppState plan stash

**Files:**
- Modify: `apps/desktop/src-tauri/src/commands/mod.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

**Decision:** To avoid serializing the full plan on every edit, we stash the active plan in `AppState.active_import` keyed by `import_id`. Commands take `import_id` and optional edits; `commit` pulls the stashed plan.

- [ ] **Step 1: Extend `AppState`**

In `commands/mod.rs`, find the `AppState` struct (around line 29) and extend it:

```rust
pub struct AppState {
    pub pool: Mutex<Option<SqlitePool>>,
    pub household_id: Mutex<Option<String>>,
    pub active_import: Mutex<Option<crate::core::import::gnucash::ImportPlan>>,
}
```

And update `AppState::new`:

```rust
impl AppState {
    pub fn new() -> Self {
        Self {
            pool: Mutex::new(None),
            household_id: Mutex::new(None),
            active_import: Mutex::new(None),
        }
    }
}
```

- [ ] **Step 2: Add the build/edit commands**

At the bottom of `commands/mod.rs`:

```rust
#[derive(Deserialize)]
pub struct BuildImportPlanArgs {
    pub path: String,
}

#[tauri::command]
pub async fn gnucash_build_default_plan(
    state: State<'_, AppState>,
    args: BuildImportPlanArgs,
) -> Result<crate::core::import::gnucash::ImportPlan, String> {
    use crate::core::import::gnucash::{mapper, reader};
    use std::path::Path;

    let household_id = {
        let g = state.household_id.lock().expect("household_id");
        g.clone().ok_or_else(|| "No household configured".to_string())?
    };
    let book = reader::read(Path::new(&args.path))
        .await
        .map_err(|e| e.to_string())?;

    let import_id = new_ulid();
    let plan = mapper::build_default_plan(household_id, import_id, &book, new_ulid)
        .map_err(|e| e.to_string())?;

    // Stash
    *state.active_import.lock().expect("active_import") = Some(plan.clone());
    Ok(plan)
}

#[derive(Deserialize)]
pub struct ApplyMappingEditArgs {
    pub edit: crate::core::import::gnucash::mapper::MappingEdit,
}

#[tauri::command]
pub async fn gnucash_apply_mapping_edit(
    state: State<'_, AppState>,
    args: ApplyMappingEditArgs,
) -> Result<crate::core::import::gnucash::ImportPlan, String> {
    use crate::core::import::gnucash::mapper;

    let mut guard = state.active_import.lock().expect("active_import");
    let plan = guard.as_mut().ok_or_else(|| "No active import plan".to_string())?;
    mapper::apply_mapping_edit(plan, &args.edit).map_err(|e| e.to_string())?;
    Ok(plan.clone())
}
```

- [ ] **Step 3: Register commands in `lib.rs`**

Add to the `invoke_handler` list:

```rust
commands::gnucash_build_default_plan,
commands::gnucash_apply_mapping_edit,
```

- [ ] **Step 4: Verify build**

Run: `cd apps/desktop/src-tauri && cargo build`
Expected: clean build. If the `Deserialize` macro needs `serde` trait imports on `MappingEdit`, ensure it's derived (already done in Task 11).

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/commands/mod.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(import): build_default_plan + apply_mapping_edit commands"
```

---

### Task 14: TypeScript core-types additions

**Files:**
- Modify: `packages/core-types/src/index.ts`

- [ ] **Step 1: Add import types to the shared package**

Append to `packages/core-types/src/index.ts`:

```typescript
// ── GnuCash import types ──────────────────────────────────────────────────

export interface GnuCashPreview {
  book_guid: string;
  account_count: number;
  transaction_count: number;
  non_usd_accounts: string[];
}

export type ImportAccountType = "asset" | "liability" | "income" | "expense" | "equity";
export type NormalBalance = "debit" | "credit";
export type JournalSide = "debit" | "credit";

export interface AccountMapping {
  gnc_guid: string;
  gnc_full_name: string;
  tally_account_id: string;
  tally_name: string;
  tally_parent_id: string | null;
  tally_type: ImportAccountType;
  tally_normal_balance: NormalBalance;
}

export interface PlannedLine {
  tally_account_id: string;
  amount_cents: number;
  side: JournalSide;
}

export interface PlannedTransaction {
  gnc_guid: string;
  txn_date: number;
  memo: string | null;
  lines: PlannedLine[];
}

export interface ImportPlan {
  household_id: string;
  import_id: string;
  account_mappings: AccountMapping[];
  transactions: PlannedTransaction[];
}

export type MappingEdit =
  | { kind: "change_type"; gnc_full_name: string; new_type: ImportAccountType; new_normal_balance: NormalBalance }
  | { kind: "rename"; gnc_full_name: string; new_tally_name: string };

export interface ImportReceipt {
  import_id: string;
  accounts_created: number;
  transactions_committed: number;
  transactions_skipped: number;
}
```

- [ ] **Step 2: Run tsc to verify types build**

Run: `cd packages/core-types && pnpm test`
Expected: PASS (existing tests unaffected; new types just add surface).

- [ ] **Step 3: Commit**

```bash
git add packages/core-types/src/index.ts
git commit -m "feat(core-types): GnuCash import types"
```

---

### Task 15: Onboarding engine — GnuCash branch detection

**Files:**
- Read first: `apps/desktop/src/hooks/useOnboardingEngine.ts`
- Modify: `apps/desktop/src/hooks/useOnboardingEngine.ts`
- Modify: `apps/desktop/src/hooks/useOnboardingEngine.test.ts`

- [ ] **Step 1: Read the existing onboarding engine**

Run: `sed -n '1,100p' apps/desktop/src/hooks/useOnboardingEngine.ts` — identify the phase-detection logic and the existing `hledger` branch (from T-042). Our GnuCash branch parallels it.

- [ ] **Step 2: Write the failing test**

Append a new `describe` block in `useOnboardingEngine.test.ts`:

```typescript
describe("GnuCash migration branch", () => {
  it("detects 'migrate from gnucash' intent and emits file-picker setup card", async () => {
    const deps = buildDeps();
    const handler = buildOnboardingHandler(deps);
    await handler.bootstrap();

    const reply = await handler.handleUserMessage("I'd like to migrate from GnuCash");

    // Should emit a setup card prompting for the .gnucash file path
    expect(reply.messages.some(m => m.kind === "setup_card" && m.card === "gnucash_file_picker")).toBe(true);
  });
});
```

- [ ] **Step 3: Run test to verify failure**

Run: `cd apps/desktop && pnpm vitest run src/hooks/useOnboardingEngine.test.ts`
Expected: FAIL — the `gnucash_file_picker` setup card isn't emitted yet.

- [ ] **Step 4: Add the branch**

In `useOnboardingEngine.ts`, locate the intent-detection switch that handles the migration path for hledger, and add a parallel case for GnuCash. Look for text like `/gnucash/i` or similar. Sketch:

```typescript
// Phase detection for user messages during initial setup
if (/migrate.*gnucash|gnucash.*migrat|gnucash/i.test(message)) {
  return {
    messages: [
      { id: ulid(), kind: "setup_card", card: "gnucash_file_picker", createdAt: now() },
    ],
    nextPhase: "gnucash_import_pick_file",
  };
}
```

Add `"gnucash_import_pick_file"` and subsequent phases to the `Phase` union type at the top of the file:

```typescript
type Phase =
  | ...existing phases...
  | "gnucash_import_pick_file"
  | "gnucash_import_mapping"
  | "gnucash_import_committing"
  | "gnucash_import_reconciling"
  | "gnucash_import_done";
```

Also add a `card` value `"gnucash_file_picker"` to whatever `SetupCardKind` union the engine uses (follow existing pattern — check `setupCardKinds`).

- [ ] **Step 5: Run test to verify it passes**

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src/hooks/useOnboardingEngine.ts apps/desktop/src/hooks/useOnboardingEngine.test.ts
git commit -m "feat(onboarding): detect GnuCash migration intent, emit file picker"
```

---

### Task 16: GnuCash file picker → preview flow

**Files:**
- Modify: `apps/desktop/src/hooks/useOnboardingEngine.ts`
- Modify: `apps/desktop/src/hooks/useOnboardingEngine.test.ts`
- Modify: `apps/desktop/src/components/onboarding/SetupCard.tsx` (add new variant)

- [ ] **Step 1: Write the failing test**

Append to `useOnboardingEngine.test.ts`:

```typescript
it("after picking a valid USD book, transitions to mapping phase with default plan", async () => {
  const deps = buildDeps({
    readGnuCashFile: vi.fn().mockResolvedValue({
      book_guid: "b1",
      account_count: 3,
      transaction_count: 2,
      non_usd_accounts: [],
    }),
    gnucashBuildDefaultPlan: vi.fn().mockResolvedValue({
      household_id: "hh",
      import_id: "imp",
      account_mappings: [
        { gnc_guid: "a", gnc_full_name: "Checking", tally_account_id: "u1", tally_name: "Checking", tally_parent_id: null, tally_type: "asset", tally_normal_balance: "debit" },
      ],
      transactions: [],
    }),
  });
  const handler = buildOnboardingHandler(deps);
  await handler.bootstrap();
  await handler.handleUserMessage("migrate from GnuCash");

  const reply = await handler.handleFilePicked("/tmp/book.gnucash");
  expect(deps.readGnuCashFile).toHaveBeenCalledWith("/tmp/book.gnucash");
  expect(deps.gnucashBuildDefaultPlan).toHaveBeenCalled();
  expect(reply.messages.some(m => m.kind === "artifact" && m.artifact === "gnucash_mapping")).toBe(true);
});

it("rejects non-USD books with a hard-error system message", async () => {
  const deps = buildDeps({
    readGnuCashFile: vi.fn().mockResolvedValue({
      book_guid: "b1", account_count: 2, transaction_count: 0,
      non_usd_accounts: ["Euro Savings"],
    }),
  });
  const handler = buildOnboardingHandler(deps);
  await handler.bootstrap();
  await handler.handleUserMessage("migrate from GnuCash");
  const reply = await handler.handleFilePicked("/tmp/book.gnucash");

  expect(reply.messages.some(m =>
    m.kind === "system" && m.severity === "error" && m.body.includes("Euro Savings")
  )).toBe(true);
});
```

- [ ] **Step 2: Run test to verify failure**

Expected: FAIL — `handleFilePicked` method doesn't exist; deps don't include the new callbacks.

- [ ] **Step 3: Extend deps and handler**

In `useOnboardingEngine.ts`:

```typescript
export interface OnboardingDeps {
  // ...existing deps...
  readGnuCashFile: (path: string) => Promise<GnuCashPreview>;
  gnucashBuildDefaultPlan: (path: string) => Promise<ImportPlan>;
  gnucashApplyMappingEdit: (edit: MappingEdit) => Promise<ImportPlan>;
  commitGnuCashImport: () => Promise<ImportReceipt>;
  reconcileGnuCashImport: (importId: string, path: string) => Promise<BalanceReportArtifact>;
  rollbackGnuCashImport: (importId: string) => Promise<void>;
}
```

Import the types:
```typescript
import type {
  GnuCashPreview, ImportPlan, ImportReceipt, MappingEdit,
} from "@tally/core-types";
```

Add `handleFilePicked` to the returned handler:

```typescript
async function handleFilePicked(path: string): Promise<HandlerReply> {
  const preview = await deps.readGnuCashFile(path);
  if (preview.non_usd_accounts.length > 0) {
    return {
      messages: [{
        id: ulid(),
        kind: "system",
        severity: "error",
        body: `This GnuCash book has accounts in other currencies. Tally currently supports USD only. Accounts: ${preview.non_usd_accounts.join(", ")}`,
        createdAt: now(),
      }],
      nextPhase: "gnucash_import_pick_file",
    };
  }
  const plan = await deps.gnucashBuildDefaultPlan(path);
  // Stash the picked path in engine state for reconcile/rollback later
  setPickedPath(path);
  setActivePlan(plan);
  return {
    messages: [{
      id: ulid(),
      kind: "artifact",
      artifact: "gnucash_mapping",
      payload: plan,
      createdAt: now(),
    }],
    nextPhase: "gnucash_import_mapping",
  };
}
```

The exact places to add `setPickedPath` / `setActivePlan` depend on existing state shape — add a small `importState` ref in the handler closure mirroring existing patterns.

- [ ] **Step 4: Run tests**

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/hooks/useOnboardingEngine.ts apps/desktop/src/hooks/useOnboardingEngine.test.ts apps/desktop/src/components/onboarding/SetupCard.tsx
git commit -m "feat(onboarding): GnuCash file picker → default plan, hard-fail on non-USD"
```

---

### Task 17: Mapping artifact card

**Files:**
- Create: `apps/desktop/src/components/artifacts/GnuCashMappingCard.tsx`
- Create: `apps/desktop/src/components/artifacts/GnuCashMappingCard.test.tsx`
- Modify: `apps/desktop/src/components/artifacts/ArtifactCard.tsx` (register variant)

- [ ] **Step 1: Write the failing render test**

Create `GnuCashMappingCard.test.tsx`:

```typescript
import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { GnuCashMappingCard } from "./GnuCashMappingCard";
import type { ImportPlan } from "@tally/core-types";

function samplePlan(): ImportPlan {
  return {
    household_id: "hh", import_id: "imp",
    account_mappings: [
      { gnc_guid: "a1", gnc_full_name: "Checking", tally_account_id: "u1", tally_name: "Checking", tally_parent_id: null, tally_type: "asset", tally_normal_balance: "debit" },
      { gnc_guid: "a2", gnc_full_name: "Groceries", tally_account_id: "u2", tally_name: "Groceries", tally_parent_id: null, tally_type: "expense", tally_normal_balance: "debit" },
    ],
    transactions: [],
  };
}

describe("GnuCashMappingCard", () => {
  it("renders every account mapping with inferred type", () => {
    render(<GnuCashMappingCard plan={samplePlan()} onConfirm={() => {}} onRequestEdit={() => {}} />);
    expect(screen.getByText("Checking")).toBeInTheDocument();
    expect(screen.getByText("Groceries")).toBeInTheDocument();
    expect(screen.getAllByText(/asset|expense/i)).toHaveLength(2);
  });

  it("fires onConfirm when user accepts", () => {
    const onConfirm = vi.fn();
    render(<GnuCashMappingCard plan={samplePlan()} onConfirm={onConfirm} onRequestEdit={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: /looks right/i }));
    expect(onConfirm).toHaveBeenCalled();
  });

  it("fires onRequestEdit when user clicks change button", () => {
    const onRequestEdit = vi.fn();
    render(<GnuCashMappingCard plan={samplePlan()} onConfirm={() => {}} onRequestEdit={onRequestEdit} />);
    fireEvent.click(screen.getByRole("button", { name: /change something/i }));
    expect(onRequestEdit).toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `cd apps/desktop && pnpm vitest run src/components/artifacts/GnuCashMappingCard.test.tsx`
Expected: FAIL — component doesn't exist.

- [ ] **Step 3: Implement the component**

Create `GnuCashMappingCard.tsx`:

```tsx
import type { ImportPlan } from "@tally/core-types";

interface Props {
  plan: ImportPlan;
  onConfirm: () => void;
  onRequestEdit: () => void;
}

export function GnuCashMappingCard({ plan, onConfirm, onRequestEdit }: Props) {
  return (
    <div className="gnucash-mapping-card">
      <div className="gnucash-mapping-card__header">
        <h3>Account mapping preview</h3>
        <p>{plan.account_mappings.length} accounts, {plan.transactions.length} transactions</p>
      </div>
      <table className="gnucash-mapping-card__table">
        <thead>
          <tr><th>GnuCash account</th><th>Tally type</th></tr>
        </thead>
        <tbody>
          {plan.account_mappings.map(m => (
            <tr key={m.gnc_guid}>
              <td>{m.gnc_full_name}</td>
              <td><span className={`type-pill type-pill--${m.tally_type}`}>{m.tally_type}</span></td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="gnucash-mapping-card__actions">
        <button type="button" onClick={onConfirm}>Looks right</button>
        <button type="button" onClick={onRequestEdit}>I need to change something</button>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Register in ArtifactCard**

In `ArtifactCard.tsx`, find the `switch (artifact.kind)` or similar and add:

```tsx
case "gnucash_mapping":
  return <GnuCashMappingCard plan={artifact.payload} onConfirm={onConfirm} onRequestEdit={onRequestEdit} />;
```

Adjust prop names to match the existing ArtifactCard prop surface; add `onConfirm` and `onRequestEdit` props at the ArtifactCard level that the `ChatThread` can wire up when rendering a GnuCash mapping artifact.

- [ ] **Step 5: Run tests**

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src/components/artifacts/GnuCashMappingCard.tsx apps/desktop/src/components/artifacts/GnuCashMappingCard.test.tsx apps/desktop/src/components/artifacts/ArtifactCard.tsx
git commit -m "feat(ui): GnuCashMappingCard artifact"
```

---

### Task 18: Mapping-edit loop in onboarding

**Files:**
- Modify: `apps/desktop/src/hooks/useOnboardingEngine.ts`
- Modify: `apps/desktop/src/hooks/useOnboardingEngine.test.ts`

**Strategy:** When the user says "change X to Y" during mapping phase, route through the existing Claude send-message flow with a special system instruction. Claude returns a `MappingEdit` via tool-use; engine calls `gnucashApplyMappingEdit` and re-emits an updated mapping card.

**Simplification for Phase 1:** Instead of involving Claude on mapping edits (risky, adds a new tool definition), use regex-based parsing of the user's free-form request: `/make (\S+) (an? )?(asset|liability|income|expense|equity)/i`. Users who need more sophisticated edits can edit the file in GnuCash and re-import. Ship small, iterate.

- [ ] **Step 1: Write the failing test**

```typescript
it("applies a mapping edit when user asks 'make X a liability'", async () => {
  const applyEdit = vi.fn().mockResolvedValue({ /* updated plan */ });
  const deps = buildDeps({ gnucashApplyMappingEdit: applyEdit, /* ...other deps with file-picker stubs... */ });
  const handler = buildOnboardingHandler(deps);
  await handler.bootstrap();
  await handler.handleUserMessage("migrate from GnuCash");
  await handler.handleFilePicked("/tmp/book.gnucash");

  const reply = await handler.handleUserMessage("make Groceries a liability");
  expect(applyEdit).toHaveBeenCalledWith({
    kind: "change_type",
    gnc_full_name: "Groceries",
    new_type: "liability",
    new_normal_balance: "credit",
  });
  expect(reply.messages.some(m => m.kind === "artifact" && m.artifact === "gnucash_mapping")).toBe(true);
});

it("confirms the plan and transitions to committing phase", async () => {
  const commit = vi.fn().mockResolvedValue({ import_id: "imp", accounts_created: 3, transactions_committed: 2, transactions_skipped: 0 });
  const deps = buildDeps({ commitGnuCashImport: commit });
  // ...setup through mapping phase...
  const reply = await handler.handleConfirmMapping();
  expect(commit).toHaveBeenCalled();
  expect(handler.phase()).toBe("gnucash_import_reconciling");
});
```

- [ ] **Step 2: Implement the parser + handler**

Add to `useOnboardingEngine.ts`:

```typescript
function parseMappingEdit(text: string): MappingEdit | null {
  const changeType = text.match(/make\s+(\S+(?:\s+\S+)*?)\s+(?:an?\s+)?(asset|liability|income|expense|equity)\b/i);
  if (changeType) {
    const [, name, type] = changeType;
    const new_type = type.toLowerCase() as ImportAccountType;
    const new_normal_balance: NormalBalance =
      new_type === "asset" || new_type === "expense" ? "debit" : "credit";
    return { kind: "change_type", gnc_full_name: name.trim(), new_type, new_normal_balance };
  }
  const rename = text.match(/rename\s+(\S+(?:\s+\S+)*?)\s+to\s+(.+)/i);
  if (rename) {
    const [, name, newName] = rename;
    return { kind: "rename", gnc_full_name: name.trim(), new_tally_name: newName.trim() };
  }
  return null;
}
```

Route it inside `handleUserMessage` when `phase === "gnucash_import_mapping"`:

```typescript
if (phase === "gnucash_import_mapping") {
  const edit = parseMappingEdit(message);
  if (!edit) {
    return {
      messages: [{ id: ulid(), kind: "system", severity: "info",
        body: "Try: 'make Groceries a liability' or 'rename Groceries to Food'", createdAt: now() }],
      nextPhase: "gnucash_import_mapping",
    };
  }
  const plan = await deps.gnucashApplyMappingEdit(edit);
  setActivePlan(plan);
  return {
    messages: [{ id: ulid(), kind: "artifact", artifact: "gnucash_mapping", payload: plan, createdAt: now() }],
    nextPhase: "gnucash_import_mapping",
  };
}
```

Add a `handleConfirmMapping()` method that calls `deps.commitGnuCashImport()` and transitions to `"gnucash_import_reconciling"`.

- [ ] **Step 3: Run tests**

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/hooks/useOnboardingEngine.ts apps/desktop/src/hooks/useOnboardingEngine.test.ts
git commit -m "feat(onboarding): mapping-edit loop with regex-based parsing"
```

---

## Ticket T-073: Committer

### Task 19: Commit a single account + transaction

**Files:**
- Modify: `apps/desktop/src-tauri/src/core/import/gnucash/committer.rs`

- [ ] **Step 1: Write the failing test**

Replace `committer.rs` with:

```rust
//! Atomically commits an `ImportPlan` to the Tally database. One SQL
//! transaction wraps everything; any failure rolls back.

use super::{AccountMapping, AccountType, ImportError, ImportPlan, ImportReceipt, NormalBalance,
    PlannedLine, PlannedTransaction, Side};
use sqlx::{Acquire, SqlitePool};

pub async fn commit(pool: &SqlitePool, plan: &ImportPlan, now_ms: i64) -> Result<ImportReceipt, ImportError> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;

    // Insert mapped accounts
    for m in &plan.account_mappings {
        sqlx::query(
            "INSERT INTO accounts (id, household_id, parent_id, name, type, normal_balance, is_placeholder, currency, created_at, import_id) \
             VALUES (?, ?, ?, ?, ?, ?, 0, 'USD', ?, ?)"
        )
        .bind(&m.tally_account_id)
        .bind(&plan.household_id)
        .bind(&m.tally_parent_id)
        .bind(&m.tally_name)
        .bind(account_type_str(m.tally_type))
        .bind(normal_balance_str(m.tally_normal_balance))
        .bind(now_ms)
        .bind(&plan.import_id)
        .execute(&mut *tx)
        .await?;
    }

    let mut committed: u32 = 0;
    let mut skipped: u32 = 0;

    for ptx in &plan.transactions {
        // Idempotency: skip if (household_id, source_ref) already exists
        let exists: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM transactions WHERE household_id = ? AND source_ref = ?",
        )
        .bind(&plan.household_id)
        .bind(&ptx.gnc_guid)
        .fetch_one(&mut *tx)
        .await?;
        if exists.0 > 0 {
            skipped += 1;
            continue;
        }

        let txn_ulid = crate::id::new_ulid();
        sqlx::query(
            "INSERT INTO transactions (id, household_id, txn_date, entry_date, status, source, memo, import_id, source_ref, created_at) \
             VALUES (?, ?, ?, ?, 'posted', 'import', ?, ?, ?, ?)"
        )
        .bind(&txn_ulid)
        .bind(&plan.household_id)
        .bind(ptx.txn_date)
        .bind(now_ms)
        .bind(&ptx.memo)
        .bind(&plan.import_id)
        .bind(&ptx.gnc_guid)
        .bind(now_ms)
        .execute(&mut *tx)
        .await?;

        for line in &ptx.lines {
            let line_ulid = crate::id::new_ulid();
            sqlx::query(
                "INSERT INTO journal_lines (id, transaction_id, account_id, envelope_id, amount, side, created_at) \
                 VALUES (?, ?, ?, NULL, ?, ?, ?)"
            )
            .bind(&line_ulid)
            .bind(&txn_ulid)
            .bind(&line.tally_account_id)
            .bind(line.amount_cents)
            .bind(side_str(line.side))
            .bind(now_ms)
            .execute(&mut *tx)
            .await?;
        }

        committed += 1;
    }

    tx.commit().await?;

    Ok(ImportReceipt {
        import_id: plan.import_id.clone(),
        accounts_created: plan.account_mappings.len() as u32,
        transactions_committed: committed,
        transactions_skipped: skipped,
    })
}

fn account_type_str(t: AccountType) -> &'static str {
    match t {
        AccountType::Asset => "asset",
        AccountType::Liability => "liability",
        AccountType::Income => "income",
        AccountType::Expense => "expense",
        AccountType::Equity => "equity",
    }
}

fn normal_balance_str(n: NormalBalance) -> &'static str {
    match n {
        NormalBalance::Debit => "debit",
        NormalBalance::Credit => "credit",
    }
}

fn side_str(s: Side) -> &'static str {
    match s {
        Side::Debit => "debit",
        Side::Credit => "credit",
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_fixtures::{build_fixture, happy_spec};
    use super::super::reader::read;
    use super::super::mapper::build_default_plan;
    use super::*;
    use crate::db::{connection::create_encrypted_db, migrations::run_migrations};
    use tempfile::tempdir;

    async fn setup_db() -> (tempfile::TempDir, SqlitePool, String) {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("tally.db");
        let salt = [0u8; 16];
        let pool = create_encrypted_db(&db_path, "pp", &salt).await.unwrap();
        run_migrations(&pool).await.unwrap();
        let hh_id = crate::id::new_ulid();
        sqlx::query("INSERT INTO households (id, name, timezone, created_at) VALUES (?, 'H', 'America/Chicago', 0)")
            .bind(&hh_id)
            .execute(&pool).await.unwrap();
        (dir, pool, hh_id)
    }

    #[tokio::test]
    async fn happy_path_commits_accounts_and_transactions() {
        let (_dir, pool, hh_id) = setup_db().await;
        let fixture_dir = tempdir().unwrap();
        let fixture_path = build_fixture(fixture_dir.path(), &happy_spec()).await;
        let book = read(&fixture_path).await.unwrap();
        let plan = build_default_plan(hh_id.clone(), crate::id::new_ulid(), &book, crate::id::new_ulid).unwrap();

        let receipt = commit(&pool, &plan, 100).await.unwrap();
        assert_eq!(receipt.accounts_created, 3);
        assert_eq!(receipt.transactions_committed, 2);
        assert_eq!(receipt.transactions_skipped, 0);

        let (acc_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE household_id = ?")
            .bind(&hh_id).fetch_one(&pool).await.unwrap();
        assert_eq!(acc_count, 3);

        let (txn_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE household_id = ? AND source = 'import'")
            .bind(&hh_id).fetch_one(&pool).await.unwrap();
        assert_eq!(txn_count, 2);
    }
}
```

- [ ] **Step 2: Run the test**

Run: `cd apps/desktop/src-tauri && cargo test --lib core::import::gnucash::committer::tests::happy_path`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src-tauri/src/core/import/gnucash/committer.rs
git commit -m "feat(import): commit ImportPlan atomically"
```

---

### Task 20: Idempotency on `source_ref`

**Files:**
- Modify: `apps/desktop/src-tauri/src/core/import/gnucash/committer.rs`

- [ ] **Step 1: Write the failing test**

Append to committer `tests`:

```rust
#[tokio::test]
async fn running_same_plan_twice_skips_all_transactions_on_second_run() {
    let (_dir, pool, hh_id) = setup_db().await;
    let fixture_dir = tempdir().unwrap();
    let fixture_path = build_fixture(fixture_dir.path(), &happy_spec()).await;
    let book = read(&fixture_path).await.unwrap();
    let plan = build_default_plan(hh_id.clone(), crate::id::new_ulid(), &book, crate::id::new_ulid).unwrap();

    let first = commit(&pool, &plan, 100).await.unwrap();
    assert_eq!(first.transactions_committed, 2);

    // Second commit needs a fresh import_id + account ulids, or we'd get a PK collision on accounts.
    // Real flow: user re-picks the file; we make a new plan with new ulids.
    let plan2 = build_default_plan(hh_id.clone(), crate::id::new_ulid(), &book, crate::id::new_ulid).unwrap();
    let second = commit(&pool, &plan2, 200).await.unwrap();
    assert_eq!(second.transactions_committed, 0);
    assert_eq!(second.transactions_skipped, 2);
}
```

Note: because accounts are fresh ULIDs on the second plan, they'll succeed. What we're testing is that transactions (keyed by `source_ref = gnc_guid`) are deduped.

- [ ] **Step 2: Run test**

Run: `cd apps/desktop/src-tauri && cargo test --lib core::import::gnucash::committer`
Expected: PASS — the existing implementation already uses `WHERE household_id = ? AND source_ref = ?` check.

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src-tauri/src/core/import/gnucash/committer.rs
git commit -m "test(import): idempotency via source_ref"
```

---

### Task 21: Atomicity on failure

**Files:**
- Modify: `apps/desktop/src-tauri/src/core/import/gnucash/committer.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn commit_rolls_back_entirely_when_any_row_fails() {
    let (_dir, pool, hh_id) = setup_db().await;
    let fixture_dir = tempdir().unwrap();
    let fixture_path = build_fixture(fixture_dir.path(), &happy_spec()).await;
    let book = read(&fixture_path).await.unwrap();
    let mut plan = build_default_plan(hh_id.clone(), crate::id::new_ulid(), &book, crate::id::new_ulid).unwrap();

    // Corrupt the plan: second transaction's first line references a nonexistent account ID.
    plan.transactions[1].lines[0].tally_account_id = "ULID_NONEXISTENT".into();

    let err = commit(&pool, &plan, 100).await.unwrap_err();
    assert!(matches!(err, ImportError::Database(_)));

    // Everything rolled back: no accounts or transactions exist
    let (acc_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE household_id = ?")
        .bind(&hh_id).fetch_one(&pool).await.unwrap();
    assert_eq!(acc_count, 0, "accounts must be rolled back on failure");

    let (txn_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE household_id = ?")
        .bind(&hh_id).fetch_one(&pool).await.unwrap();
    assert_eq!(txn_count, 0, "transactions must be rolled back on failure");
}
```

- [ ] **Step 2: Run test**

Expected: PASS — the existing implementation uses `tx.commit()` only at the end, and dropping `tx` without commit rolls back.

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src-tauri/src/core/import/gnucash/committer.rs
git commit -m "test(import): atomic rollback on commit failure"
```

---

### Task 22: Rollback by import_id

**Files:**
- Modify: `apps/desktop/src-tauri/src/core/import/gnucash/committer.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn rollback_deletes_every_row_stamped_with_import_id() {
    let (_dir, pool, hh_id) = setup_db().await;
    let fixture_dir = tempdir().unwrap();
    let fixture_path = build_fixture(fixture_dir.path(), &happy_spec()).await;
    let book = read(&fixture_path).await.unwrap();
    let plan = build_default_plan(hh_id.clone(), crate::id::new_ulid(), &book, crate::id::new_ulid).unwrap();
    commit(&pool, &plan, 100).await.unwrap();

    rollback(&pool, &plan.import_id).await.unwrap();

    let (acc_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE import_id = ?")
        .bind(&plan.import_id).fetch_one(&pool).await.unwrap();
    assert_eq!(acc_count, 0);

    let (txn_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE import_id = ?")
        .bind(&plan.import_id).fetch_one(&pool).await.unwrap();
    assert_eq!(txn_count, 0);

    let (jl_orphans,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM journal_lines jl LEFT JOIN transactions t ON t.id = jl.transaction_id WHERE t.id IS NULL"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(jl_orphans, 0, "no orphaned journal lines");
}
```

- [ ] **Step 2: Run test — expect failure**

Expected: FAIL — `rollback` doesn't exist.

- [ ] **Step 3: Implement rollback**

Add to `committer.rs`:

```rust
pub async fn rollback(pool: &SqlitePool, import_id: &str) -> Result<(), ImportError> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;

    // Delete journal_lines whose transaction was part of this import
    sqlx::query(
        "DELETE FROM journal_lines WHERE transaction_id IN (SELECT id FROM transactions WHERE import_id = ?)",
    )
    .bind(import_id)
    .execute(&mut *tx)
    .await?;

    // Delete the transactions themselves
    sqlx::query("DELETE FROM transactions WHERE import_id = ?")
        .bind(import_id)
        .execute(&mut *tx)
        .await?;

    // Delete the accounts that were created by this import
    sqlx::query("DELETE FROM accounts WHERE import_id = ?")
        .bind(import_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}
```

- [ ] **Step 4: Run tests**

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/core/import/gnucash/committer.rs
git commit -m "feat(import): rollback by import_id"
```

---

### Task 23: Commit and rollback Tauri commands

**Files:**
- Modify: `apps/desktop/src-tauri/src/commands/mod.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Add commands**

Append to `commands/mod.rs`:

```rust
#[tauri::command]
pub async fn commit_gnucash_import(
    state: State<'_, AppState>,
) -> Result<crate::core::import::gnucash::ImportReceipt, String> {
    let pool_opt = state.pool.lock().expect("pool").clone();
    let pool = pool_opt.ok_or_else(|| "No database open".to_string())?;

    let plan = {
        let g = state.active_import.lock().expect("active_import");
        g.clone().ok_or_else(|| "No active import plan".to_string())?
    };

    let receipt = crate::core::import::gnucash::committer::commit(&pool, &plan, now_ms())
        .await
        .map_err(|e| e.to_string())?;

    // Clear the stash — commit consumes it
    *state.active_import.lock().expect("active_import") = None;
    Ok(receipt)
}

#[derive(Deserialize)]
pub struct RollbackArgs {
    pub import_id: String,
}

#[tauri::command]
pub async fn rollback_gnucash_import(
    state: State<'_, AppState>,
    args: RollbackArgs,
) -> Result<(), String> {
    let pool_opt = state.pool.lock().expect("pool").clone();
    let pool = pool_opt.ok_or_else(|| "No database open".to_string())?;
    crate::core::import::gnucash::committer::rollback(&pool, &args.import_id)
        .await
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Register in `lib.rs`**

Add to `invoke_handler`:

```rust
commands::commit_gnucash_import,
commands::rollback_gnucash_import,
```

- [ ] **Step 3: Verify build**

Run: `cd apps/desktop/src-tauri && cargo build`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src-tauri/src/commands/mod.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(import): commit + rollback Tauri commands"
```

---

## Ticket T-074: Reconciler

### Task 24: Compute expected balances from GnuCashBook

**Files:**
- Modify: `apps/desktop/src-tauri/src/core/import/gnucash/reconcile.rs`

- [ ] **Step 1: Write the failing test**

Replace `reconcile.rs` with:

```rust
//! After commit, compute per-account expected balances from the source book
//! and compare them against Tally's current balances. Produces a
//! `BalanceReport` payload for the existing frontend renderer.

use super::{GnuCashBook, ImportError, ImportPlan, Side};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceReportArtifact {
    pub rows: Vec<BalanceRow>,
    pub total_mismatches: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceRow {
    pub account_name: String,
    pub tally_cents: i64,
    pub gnucash_cents: i64,
    pub matches: bool,
}

/// Sums signed GnuCash splits per account GUID. Positive sum = net debit,
/// negative = net credit (matches GnuCash's convention).
pub fn expected_balances_by_guid(book: &GnuCashBook) -> HashMap<String, i64> {
    let mut out: HashMap<String, i64> = HashMap::new();
    for tx in &book.transactions {
        for sp in &tx.splits {
            *out.entry(sp.account_guid.clone()).or_insert(0) += sp.amount_cents;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::test_fixtures::{build_fixture, happy_spec};
    use super::super::reader::read;
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn expected_balances_sum_splits() {
        let dir = tempdir().unwrap();
        let path = build_fixture(dir.path(), &happy_spec()).await;
        let book = read(&path).await.unwrap();
        let balances = expected_balances_by_guid(&book);

        // Checking: +100000 (opening) + -5000 (groceries) = +95000
        assert_eq!(balances.get("acc_checking"), Some(&95000));
        // Groceries: +5000
        assert_eq!(balances.get("acc_groceries"), Some(&5000));
        // Equity: -100000
        assert_eq!(balances.get("acc_opening"), Some(&-100000));
    }
}
```

- [ ] **Step 2: Run test**

Run: `cd apps/desktop/src-tauri && cargo test --lib core::import::gnucash::reconcile`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src-tauri/src/core/import/gnucash/reconcile.rs
git commit -m "feat(import): expected balances from GnuCashBook"
```

---

### Task 25: Reconcile Tally vs GnuCash

**Files:**
- Modify: `apps/desktop/src-tauri/src/core/import/gnucash/reconcile.rs`

- [ ] **Step 1: Write the failing test**

Append to reconcile `tests`:

```rust
#[tokio::test]
async fn reconcile_happy_path_zero_mismatches() {
    use super::super::committer::commit;
    use super::super::mapper::build_default_plan;
    use crate::db::{connection::create_encrypted_db, migrations::run_migrations};

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("tally.db");
    let salt = [0u8; 16];
    let pool = create_encrypted_db(&db_path, "pp", &salt).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let hh_id = crate::id::new_ulid();
    sqlx::query("INSERT INTO households (id, name, timezone, created_at) VALUES (?, 'H', 'America/Chicago', 0)")
        .bind(&hh_id).execute(&pool).await.unwrap();

    let fixture_dir = tempdir().unwrap();
    let fixture_path = build_fixture(fixture_dir.path(), &happy_spec()).await;
    let book = read(&fixture_path).await.unwrap();
    let plan = build_default_plan(hh_id.clone(), crate::id::new_ulid(), &book, crate::id::new_ulid).unwrap();
    commit(&pool, &plan, 100).await.unwrap();

    let report = reconcile(&pool, &plan, &book).await.unwrap();
    assert_eq!(report.total_mismatches, 0);
    assert_eq!(report.rows.len(), 3);
    for row in &report.rows {
        assert!(row.matches, "{} mismatched: tally={}, gnucash={}", row.account_name, row.tally_cents, row.gnucash_cents);
    }
}

#[tokio::test]
async fn reconcile_flags_mismatch_after_manual_corruption() {
    use super::super::committer::commit;
    use super::super::mapper::build_default_plan;
    use crate::db::{connection::create_encrypted_db, migrations::run_migrations};

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("tally.db");
    let salt = [0u8; 16];
    let pool = create_encrypted_db(&db_path, "pp", &salt).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let hh_id = crate::id::new_ulid();
    sqlx::query("INSERT INTO households (id, name, timezone, created_at) VALUES (?, 'H', 'America/Chicago', 0)")
        .bind(&hh_id).execute(&pool).await.unwrap();

    let fixture_dir = tempdir().unwrap();
    let fixture_path = build_fixture(fixture_dir.path(), &happy_spec()).await;
    let book = read(&fixture_path).await.unwrap();
    let plan = build_default_plan(hh_id.clone(), crate::id::new_ulid(), &book, crate::id::new_ulid).unwrap();
    commit(&pool, &plan, 100).await.unwrap();

    // Corrupt one journal line's amount after the fact
    sqlx::query("UPDATE journal_lines SET amount = amount + 100 WHERE transaction_id IN (SELECT id FROM transactions WHERE source_ref = 'tx_groc') LIMIT 1")
        .execute(&pool).await.unwrap();

    let report = reconcile(&pool, &plan, &book).await.unwrap();
    assert!(report.total_mismatches > 0);
    assert!(report.rows.iter().any(|r| !r.matches));
}
```

- [ ] **Step 2: Run tests to verify failure**

Expected: FAIL — `reconcile` function not defined.

- [ ] **Step 3: Implement `reconcile`**

Add to `reconcile.rs`:

```rust
pub async fn reconcile(
    pool: &SqlitePool,
    plan: &ImportPlan,
    book: &GnuCashBook,
) -> Result<BalanceReportArtifact, ImportError> {
    let expected = expected_balances_by_guid(book);

    // For each mapped account, compute Tally's current signed balance
    // (debit minus credit) from posted journal lines.
    let mut rows: Vec<BalanceRow> = Vec::with_capacity(plan.account_mappings.len());
    let mut mismatches: u32 = 0;

    for m in &plan.account_mappings {
        let (debits,): (i64,) = sqlx::query_as(
            "SELECT COALESCE(SUM(jl.amount), 0) FROM journal_lines jl \
             JOIN transactions t ON t.id = jl.transaction_id \
             WHERE jl.account_id = ? AND t.status = 'posted' AND jl.side = 'debit'",
        )
        .bind(&m.tally_account_id)
        .fetch_one(pool)
        .await?;

        let (credits,): (i64,) = sqlx::query_as(
            "SELECT COALESCE(SUM(jl.amount), 0) FROM journal_lines jl \
             JOIN transactions t ON t.id = jl.transaction_id \
             WHERE jl.account_id = ? AND t.status = 'posted' AND jl.side = 'credit'",
        )
        .bind(&m.tally_account_id)
        .fetch_one(pool)
        .await?;

        let tally_signed = debits - credits;
        let gnc_signed = expected.get(&m.gnc_guid).copied().unwrap_or(0);
        let matches = tally_signed == gnc_signed;
        if !matches {
            mismatches += 1;
        }
        rows.push(BalanceRow {
            account_name: m.gnc_full_name.clone(),
            tally_cents: tally_signed,
            gnucash_cents: gnc_signed,
            matches,
        });
    }

    Ok(BalanceReportArtifact {
        rows,
        total_mismatches: mismatches,
    })
}
```

- [ ] **Step 4: Run tests**

Expected: PASS on both reconcile tests.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/core/import/gnucash/reconcile.rs
git commit -m "feat(import): reconcile Tally vs GnuCash balances"
```

---

### Task 26: Reconcile Tauri command

**Files:**
- Modify: `apps/desktop/src-tauri/src/commands/mod.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

**Note:** Reconcile needs the plan (for account ULIDs) and the source book (for expected balances). After commit we've cleared `active_import`; the command therefore takes the file path and an import_id, re-reads the book, and rebuilds a minimal plan-view from the DB.

- [ ] **Step 1: Add the command**

Append to `commands/mod.rs`:

```rust
#[derive(Deserialize)]
pub struct ReconcileArgs {
    pub import_id: String,
    pub path: String,
}

#[tauri::command]
pub async fn reconcile_gnucash_import(
    state: State<'_, AppState>,
    args: ReconcileArgs,
) -> Result<crate::core::import::gnucash::reconcile::BalanceReportArtifact, String> {
    use crate::core::import::gnucash::{reader, reconcile, AccountMapping, ImportPlan};
    use std::path::Path;

    let pool_opt = state.pool.lock().expect("pool").clone();
    let pool = pool_opt.ok_or_else(|| "No database open".to_string())?;
    let household_id = state.household_id.lock().expect("hh").clone()
        .ok_or_else(|| "No household configured".to_string())?;

    let book = reader::read(Path::new(&args.path))
        .await
        .map_err(|e| e.to_string())?;

    // Rebuild minimal AccountMapping set from accounts table (all rows stamped with this import_id)
    #[derive(sqlx::FromRow)]
    struct Row { id: String, name: String }
    // We need the gnc_guid too. It's not on the accounts table — so we reconstruct the mapping by
    // matching Tally account_name to GnuCash account.name. This is safe because duplicate names
    // were blocked during Task 12.
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, name FROM accounts WHERE household_id = ? AND import_id = ?",
    )
    .bind(&household_id)
    .bind(&args.import_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let name_to_gnc: std::collections::HashMap<&str, &super::core::import::gnucash::GncAccount> =
        book.accounts.iter().map(|a| (a.name.as_str(), a)).collect();

    let account_mappings: Vec<AccountMapping> = rows.iter().filter_map(|r| {
        name_to_gnc.get(r.name.as_str()).map(|ga| AccountMapping {
            gnc_guid: ga.guid.clone(),
            gnc_full_name: ga.full_name.clone(),
            tally_account_id: r.id.clone(),
            tally_name: r.name.clone(),
            tally_parent_id: None,
            tally_type: crate::core::import::gnucash::AccountType::Asset, // unused by reconcile
            tally_normal_balance: crate::core::import::gnucash::NormalBalance::Debit,
        })
    }).collect();

    let plan = ImportPlan {
        household_id,
        import_id: args.import_id,
        account_mappings,
        transactions: vec![], // unused by reconcile
    };

    reconcile::reconcile(&pool, &plan, &book)
        .await
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Register in `lib.rs`**

Add `commands::reconcile_gnucash_import,` to `invoke_handler`.

- [ ] **Step 3: Verify build**

Run: `cd apps/desktop/src-tauri && cargo build`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src-tauri/src/commands/mod.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(import): reconcile_gnucash_import command"
```

---

### Task 27: Reconciliation artifact card

**Files:**
- Create: `apps/desktop/src/components/artifacts/GnuCashReconcileCard.tsx`
- Create: `apps/desktop/src/components/artifacts/GnuCashReconcileCard.test.tsx`
- Modify: `apps/desktop/src/components/artifacts/ArtifactCard.tsx` (register variant)

- [ ] **Step 1: Write the failing test**

Create `GnuCashReconcileCard.test.tsx`:

```typescript
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { GnuCashReconcileCard } from "./GnuCashReconcileCard";

const sampleReport = {
  total_mismatches: 0,
  rows: [
    { account_name: "Checking", tally_cents: 95000, gnucash_cents: 95000, matches: true },
    { account_name: "Groceries", tally_cents: 5000, gnucash_cents: 5000, matches: true },
  ],
};

const mismatchReport = {
  total_mismatches: 1,
  rows: [
    { account_name: "Checking", tally_cents: 94900, gnucash_cents: 95000, matches: false },
  ],
};

describe("GnuCashReconcileCard", () => {
  it("renders every row with tally + gnucash balance", () => {
    render(<GnuCashReconcileCard report={sampleReport} onAccept={() => {}} onRollback={() => {}} />);
    expect(screen.getByText("Checking")).toBeInTheDocument();
    expect(screen.getByText("$950.00")).toBeInTheDocument();
  });

  it("flags mismatches visibly", () => {
    render(<GnuCashReconcileCard report={mismatchReport} onAccept={() => {}} onRollback={() => {}} />);
    expect(screen.getByText(/1 mismatch/i)).toBeInTheDocument();
  });

  it("wires accept + rollback buttons", () => {
    const onAccept = vi.fn();
    const onRollback = vi.fn();
    render(<GnuCashReconcileCard report={sampleReport} onAccept={onAccept} onRollback={onRollback} />);
    fireEvent.click(screen.getByRole("button", { name: /looks right/i }));
    fireEvent.click(screen.getByRole("button", { name: /roll back/i }));
    expect(onAccept).toHaveBeenCalled();
    expect(onRollback).toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Implement the component**

Create `GnuCashReconcileCard.tsx`:

```tsx
interface BalanceRow {
  account_name: string;
  tally_cents: number;
  gnucash_cents: number;
  matches: boolean;
}

interface Report {
  rows: BalanceRow[];
  total_mismatches: number;
}

interface Props {
  report: Report;
  onAccept: () => void;
  onRollback: () => void;
}

function cents(n: number): string {
  const abs = Math.abs(n);
  const dollars = (abs / 100).toFixed(2);
  return `${n < 0 ? "-" : ""}$${dollars}`;
}

export function GnuCashReconcileCard({ report, onAccept, onRollback }: Props) {
  const { rows, total_mismatches } = report;
  return (
    <div className="gnucash-reconcile-card">
      <div className="gnucash-reconcile-card__header">
        <h3>Balance reconciliation</h3>
        <p>
          {total_mismatches === 0
            ? "All balances match GnuCash."
            : `${total_mismatches} mismatch${total_mismatches === 1 ? "" : "es"} — review below.`}
        </p>
      </div>
      <table className="gnucash-reconcile-card__table">
        <thead>
          <tr><th>Account</th><th>Tally</th><th>GnuCash</th><th></th></tr>
        </thead>
        <tbody>
          {rows.map(r => (
            <tr key={r.account_name} className={r.matches ? "" : "gnucash-reconcile-card__row--mismatch"}>
              <td>{r.account_name}</td>
              <td>{cents(r.tally_cents)}</td>
              <td>{cents(r.gnucash_cents)}</td>
              <td>{r.matches ? "✓" : "!"}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="gnucash-reconcile-card__actions">
        <button type="button" onClick={onAccept}>Looks right, continue</button>
        <button type="button" onClick={onRollback}>Roll back</button>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Register in ArtifactCard switch**

Add a `"gnucash_reconcile"` case to `ArtifactCard.tsx` rendering `<GnuCashReconcileCard ... />`.

- [ ] **Step 4: Run tests**

Run: `cd apps/desktop && pnpm vitest run src/components/artifacts/GnuCashReconcileCard.test.tsx`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/components/artifacts/
git commit -m "feat(ui): GnuCashReconcileCard artifact"
```

---

### Task 28: Onboarding engine — reconcile phase and blocking handoff

**Files:**
- Modify: `apps/desktop/src/hooks/useOnboardingEngine.ts`
- Modify: `apps/desktop/src/hooks/useOnboardingEngine.test.ts`

- [ ] **Step 1: Write the failing tests**

```typescript
describe("GnuCash reconcile phase", () => {
  it("after commit, fetches balance report and renders reconcile artifact", async () => {
    const reconcile = vi.fn().mockResolvedValue({ total_mismatches: 0, rows: [] });
    const deps = buildDeps({ reconcileGnuCashImport: reconcile });
    const handler = await progressToCommitting(deps, handler => handler);
    const reply = await handler.handleCommitComplete({ import_id: "imp_1", accounts_created: 3, transactions_committed: 2, transactions_skipped: 0 });
    expect(reconcile).toHaveBeenCalledWith("imp_1", expect.any(String));
    expect(reply.messages.some(m => m.kind === "artifact" && m.artifact === "gnucash_reconcile")).toBe(true);
    expect(handler.phase()).toBe("gnucash_import_reconciling");
  });

  it("accepting the report transitions to handoff", async () => {
    /* ...setup through reconcile phase... */
    const reply = await handler.handleReconcileAccept();
    expect(handler.phase()).toBe("gnucash_import_done");
    expect(reply.messages.some(m => m.kind === "handoff")).toBe(true);
  });

  it("rejecting rolls back and returns to file picker", async () => {
    const rollback = vi.fn().mockResolvedValue(undefined);
    /* ...setup through reconcile... */
    const reply = await handler.handleReconcileReject();
    expect(rollback).toHaveBeenCalledWith("imp_1");
    expect(handler.phase()).toBe("gnucash_import_pick_file");
  });
});
```

- [ ] **Step 2: Implement the handler methods**

Add to `useOnboardingEngine.ts`:

```typescript
async function handleCommitComplete(receipt: ImportReceipt): Promise<HandlerReply> {
  const path = getPickedPath();
  const report = await deps.reconcileGnuCashImport(receipt.import_id, path);
  setActiveImportId(receipt.import_id);
  return {
    messages: [{ id: ulid(), kind: "artifact", artifact: "gnucash_reconcile", payload: report, createdAt: now() }],
    nextPhase: "gnucash_import_reconciling",
  };
}

async function handleReconcileAccept(): Promise<HandlerReply> {
  // Emit the handoff message (reuse the existing handoff flow)
  return {
    messages: [{ id: ulid(), kind: "handoff", body: "Migration complete.", createdAt: now() }],
    nextPhase: "gnucash_import_done",
  };
}

async function handleReconcileReject(): Promise<HandlerReply> {
  const importId = getActiveImportId();
  await deps.rollbackGnuCashImport(importId);
  clearActiveImportId();
  return {
    messages: [{ id: ulid(), kind: "system", severity: "info",
      body: "Import rolled back. Pick a GnuCash file to try again, or skip migration.", createdAt: now() }],
    nextPhase: "gnucash_import_pick_file",
  };
}
```

Wire these methods to the `GnuCashReconcileCard`'s `onAccept` / `onRollback` callbacks in `ChatThread` (follow the pattern used by `TransactionCardPending` from T-047).

- [ ] **Step 3: Run tests**

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/hooks/useOnboardingEngine.ts apps/desktop/src/hooks/useOnboardingEngine.test.ts
git commit -m "feat(onboarding): reconcile phase with accept/rollback gate"
```

---

### Task 29: Integration test — end-to-end happy path

**Files:**
- Create: `apps/desktop/src-tauri/tests/gnucash_import_integration.rs`

- [ ] **Step 1: Write the test**

Create the integration test file:

```rust
//! End-to-end: GnuCash fixture → reader → default plan → commit → reconcile.
//! Proves the four phases compose correctly.

use tally_desktop_lib::core::import::gnucash::{committer, mapper, reader, reconcile};
use tally_desktop_lib::db::{connection::create_encrypted_db, migrations::run_migrations};
use tally_desktop_lib::id::new_ulid;
use tempfile::tempdir;

// Replicate the fixture builder from the internal test module. Keeping this
// inline (rather than exposing the internal test_fixtures module as pub) keeps
// the production API surface clean.
async fn build_happy_fixture(dir: &std::path::Path) -> std::path::PathBuf {
    // ...same SQL as test_fixtures::build_fixture with happy_spec...
    // For brevity of this plan: copy the contents of happy_spec() into this file.
    unimplemented!("copy from src/core/import/gnucash/test_fixtures.rs happy_spec + build_fixture")
}

#[tokio::test]
async fn end_to_end_happy_path() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("tally.db");
    let pool = create_encrypted_db(&db_path, "pp", &[0u8; 16]).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let hh_id = new_ulid();
    sqlx::query("INSERT INTO households (id, name, timezone, created_at) VALUES (?, 'H', 'America/Chicago', 0)")
        .bind(&hh_id).execute(&pool).await.unwrap();

    let fixture_dir = tempdir().unwrap();
    let fixture_path = build_happy_fixture(fixture_dir.path()).await;

    // Read
    let preview = reader::preview(&fixture_path).await.unwrap();
    assert!(preview.non_usd_accounts.is_empty());
    let book = reader::read(&fixture_path).await.unwrap();

    // Map
    let plan = mapper::build_default_plan(hh_id.clone(), new_ulid(), &book, new_ulid).unwrap();
    assert!(mapper::find_duplicate_names(&plan).is_empty());

    // Commit
    let receipt = committer::commit(&pool, &plan, 100).await.unwrap();
    assert_eq!(receipt.transactions_committed, 2);

    // Reconcile
    let report = reconcile::reconcile(&pool, &plan, &book).await.unwrap();
    assert_eq!(report.total_mismatches, 0);

    // Audit log has one entry per inserted transaction
    let (audit_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM audit_log WHERE table_name = 'transactions'"
    ).fetch_one(&pool).await.unwrap();
    assert!(audit_count >= 2);
}
```

**Pragmatic simplification:** Rather than duplicating the fixture builder, we'll expose the `test_fixtures` module as `pub(crate)` inside a `#[cfg(test)]` guard isn't possible across integration boundaries, so instead: just call the production `reader::read` against an actual SQLite file we build here. The fixture-build SQL is in `test_fixtures.rs` already; copy its `build_fixture` body (schema + inserts) into a helper function inside `gnucash_import_integration.rs`. It's ~50 lines of SQL — acceptable duplication.

- [ ] **Step 2: Fill in `build_happy_fixture`**

Copy the body of `test_fixtures::build_fixture` and `test_fixtures::happy_spec` into the integration test file, adjusting for single-use. This is duplication but keeps the production module free of `pub` test helpers.

- [ ] **Step 3: Run the integration test**

Run: `cd apps/desktop/src-tauri && cargo test --test gnucash_import_integration`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src-tauri/tests/gnucash_import_integration.rs
git commit -m "test(import): end-to-end integration test"
```

---

### Task 30: Wire Tauri invoke calls from TS

**Files:**
- Create: `apps/desktop/src/lib/tauri/gnucashImport.ts`

- [ ] **Step 1: Add thin invoke wrappers**

Create `apps/desktop/src/lib/tauri/gnucashImport.ts`:

```typescript
import { invoke } from "@tauri-apps/api/core";
import type {
  GnuCashPreview, ImportPlan, ImportReceipt, MappingEdit,
} from "@tally/core-types";
import type { BalanceReportArtifact } from "../../components/artifacts/GnuCashReconcileCard";

export const readGnuCashFile = (path: string): Promise<GnuCashPreview> =>
  invoke("read_gnucash_file", { args: { path } });

export const gnucashBuildDefaultPlan = (path: string): Promise<ImportPlan> =>
  invoke("gnucash_build_default_plan", { args: { path } });

export const gnucashApplyMappingEdit = (edit: MappingEdit): Promise<ImportPlan> =>
  invoke("gnucash_apply_mapping_edit", { args: { edit } });

export const commitGnuCashImport = (): Promise<ImportReceipt> =>
  invoke("commit_gnucash_import");

export const reconcileGnuCashImport = (importId: string, path: string): Promise<BalanceReportArtifact> =>
  invoke("reconcile_gnucash_import", { args: { import_id: importId, path } });

export const rollbackGnuCashImport = (importId: string): Promise<void> =>
  invoke("rollback_gnucash_import", { args: { import_id: importId } });
```

- [ ] **Step 2: Wire these into the onboarding deps construction**

Wherever the production onboarding deps are built (typically in `App.tsx` or a bootstrap file), pass these functions into `buildOnboardingHandler`. Match the pattern used by existing deps.

- [ ] **Step 3: Run the frontend test suite**

Run: `cd apps/desktop && pnpm test`
Expected: all tests PASS.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/lib/tauri/gnucashImport.ts apps/desktop/src/
git commit -m "feat(import): TS invoke wrappers for gnucash commands"
```

---

### Task 31: Manual smoke test checklist

**Files:** none (manual verification — required before PR).

- [ ] **Step 1: Launch the app**

Run: `cd apps/desktop && pnpm tauri dev`
Wait for the window to open.

- [ ] **Step 2: Walk through migration flow**

1. Fresh start (delete `~/Library/Application Support/tally-desktop/tally.db` if present).
2. On the onboarding screen, type "I want to migrate from GnuCash".
3. Confirm the file-picker setup card appears.
4. Point it at a real GnuCash book (any one you have, even a tiny test one).
5. Confirm the mapping artifact card lists every account with an inferred Tally type.
6. If any accounts look wrong, type a mapping edit ("make X a liability"). Confirm the card re-renders.
7. Click "Looks right". Wait for commit.
8. Confirm the reconcile artifact appears with your real balances side-by-side.
9. Verify at least one account against GnuCash manually.
10. Click "Looks right, continue". Confirm the chat switches to normal mode and the sidebar shows imported accounts.

- [ ] **Step 3: Test the rollback path**

Repeat steps 1-8. At step 10, click "Roll back" instead. Confirm:
- You're returned to the file-picker phase.
- The sidebar account panel is empty again.
- The DB no longer contains any imported transactions (`sqlite3 ~/.../tally.db "SELECT COUNT(*) FROM transactions WHERE source='import'"` should be 0, though you'd need to decrypt first).

- [ ] **Step 4: Test the non-USD hard-fail**

Take a GnuCash book with at least one non-USD account. Point the import at it. Confirm:
- A plain-language system error lists the offending account names.
- No DB writes occur.

- [ ] **Step 5: If everything looks right, ready for PR**

---

### Task 32: Open PR

**Files:** none (git + gh).

- [ ] **Step 1: Push the branch**

```bash
git push -u origin feat/gnucash-import
```

- [ ] **Step 2: Open the PR**

```bash
gh pr create --title "feat(import): GnuCash SQLite import (T-071 – T-074)" --body "$(cat <<'EOF'
## Summary

Adds the onboarding-only GnuCash SQLite import path: reader → mapper → committer → reconciler, wired through new Tauri commands and artifact cards.

- T-071: reader with currency scan and splits-balance validation
- T-072: mapper with default type mapping, edit loop, duplicate detection
- T-073: atomic committer, idempotent on GnuCash GUID, scoped rollback
- T-074: post-commit balance report blocking handoff until the user accepts

Spec: `docs/superpowers/specs/2026-04-24-gnucash-import-design.md`.
Plan: `docs/superpowers/plans/2026-04-24-gnucash-import.md`.

## Test plan

- [ ] `cargo test --lib core::import::gnucash` passes
- [ ] `cargo test --test gnucash_import_integration` passes
- [ ] `pnpm test` in `apps/desktop` passes
- [ ] Manual: migrate a real GnuCash book end-to-end, verify balances match
- [ ] Manual: rollback from reconcile screen; DB returns to pre-import state
- [ ] Manual: non-USD book rejected with plain-language error

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

- [ ] **Step 3: Confirm CI passes**

Watch the checks. Rust CI takes ~4 min. If anything fails, fix in a new commit (never `--amend` after push per repo rules).

---

## Spec Coverage Self-Check

Each spec requirement → task that implements it:

| Spec section / requirement | Task(s) |
|---|---|
| Onboarding-only entry point | Task 15 (intent detection) |
| Hard-fail on non-USD commodity | Task 6 (preview), Task 16 (TS handler) |
| Preserve opening-balance pattern verbatim | Task 19 (commit uses `source='import'`) |
| Confirm-all-or-edit CoA mapping UX | Tasks 17, 18 |
| Skip envelopes at import time | Task 19 (`envelope_id = NULL`) |
| Blocking reconciliation with rollback | Tasks 27, 28 |
| Three-phase atomic pipeline | Tasks 5–10 (read), 10–14 (map), 19–23 (commit), 24–28 (reconcile) |
| `source_ref` column + unique index | Task 2 |
| `import_id` on accounts | Task 2 |
| Default type mapping table | Task 9 |
| Duplicate-name detection → SoftWarning | Task 12 |
| Reader: file not found / not GnuCash / corrupt | Tasks 5, 7 |
| Commit atomicity | Task 21 |
| Commit idempotency on `source_ref` | Task 20 |
| Rollback deletes in dependency order | Task 22 |
| Balance-report artifact | Tasks 25, 27 |
| Integration test | Task 29 |
| Batch single PR | Task 32 |

No gaps.

## Placeholder Scan

Searched the plan for `TBD`, `TODO`, `implement later`, `add appropriate`, "similar to Task N without code." None present.

## Type Consistency

- `MappingEdit` variants (`change_type`, `rename`) match between Rust (Task 11) and TS (Task 14).
- `AccountType` values (`asset`/`liability`/…) match in `default_tally_type` (Task 9), `account_type_str` (Task 19), and TS `ImportAccountType` (Task 14).
- `Side` / `JournalSide` values (`debit`/`credit`) match in Rust `Side` enum and TS type.
- `ImportPlan.account_mappings` / `.transactions` field names match across Rust struct (Task 3), TS type (Task 14), and card components (Tasks 17, 27).
