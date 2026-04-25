//! End-to-end: GnuCash fixture → reader → default plan → commit → reconcile.
//! Proves the four phases compose correctly against the real encrypted Tally DB.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::str::FromStr;
use tally_desktop_lib::core::import::gnucash::{committer, mapper, reader, reconcile};
use tally_desktop_lib::db::create_encrypted_db;
use tally_desktop_lib::id::new_ulid;
use tempfile::tempdir;

/// Builds a small GnuCash fixture in `dir` and returns its path.
/// Mirrors the internal `test_fixtures::build_fixture` helper — we duplicate
/// because integration tests can't see `#[cfg(test)]` modules.
async fn build_happy_fixture(dir: &std::path::Path) -> PathBuf {
    let path = dir.join("happy.gnucash");
    let url = format!("sqlite://{}?mode=rwc", path.display());
    let opts = SqliteConnectOptions::from_str(&url).expect("valid sqlite url");
    let pool: SqlitePool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("open fixture db");

    // Create GnuCash schema (minimal subset our reader queries).
    for stmt in [
        "CREATE TABLE books (guid TEXT PRIMARY KEY NOT NULL, root_account_guid TEXT, root_template_guid TEXT)",
        "CREATE TABLE commodities (guid TEXT PRIMARY KEY NOT NULL, namespace TEXT NOT NULL, mnemonic TEXT NOT NULL, fullname TEXT, cusip TEXT, fraction INTEGER NOT NULL, quote_flag INTEGER NOT NULL)",
        "CREATE TABLE accounts (guid TEXT PRIMARY KEY NOT NULL, name TEXT NOT NULL, account_type TEXT NOT NULL, commodity_guid TEXT, commodity_scu INTEGER NOT NULL, non_std_scu INTEGER NOT NULL, parent_guid TEXT, code TEXT, description TEXT, hidden INTEGER, placeholder INTEGER)",
        "CREATE TABLE transactions (guid TEXT PRIMARY KEY NOT NULL, currency_guid TEXT NOT NULL, num TEXT NOT NULL, post_date TEXT, enter_date TEXT, description TEXT)",
        "CREATE TABLE splits (guid TEXT PRIMARY KEY NOT NULL, tx_guid TEXT NOT NULL, account_guid TEXT NOT NULL, memo TEXT NOT NULL, action TEXT NOT NULL, reconcile_state TEXT NOT NULL, reconcile_date TEXT, value_num INTEGER NOT NULL, value_denom INTEGER NOT NULL, quantity_num INTEGER NOT NULL, quantity_denom INTEGER NOT NULL, lot_guid TEXT)",
    ] {
        sqlx::query(stmt).execute(&pool).await.expect("create schema");
    }

    sqlx::query("INSERT INTO books (guid, root_account_guid) VALUES ('book_int', NULL)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO commodities (guid, namespace, mnemonic, fraction, quote_flag) VALUES ('cmdty_usd', 'CURRENCY', 'USD', 100, 0)")
        .execute(&pool).await.unwrap();

    // 3 accounts: Checking (BANK), Groceries (EXPENSE), Opening Balances (EQUITY)
    for (guid, name, acc_type) in [
        ("acc_checking", "Checking", "BANK"),
        ("acc_groceries", "Groceries", "EXPENSE"),
        ("acc_opening", "Opening Balances", "EQUITY"),
    ] {
        sqlx::query(
            "INSERT INTO accounts (guid, name, account_type, commodity_guid, commodity_scu, non_std_scu, parent_guid, hidden, placeholder) VALUES (?, ?, ?, 'cmdty_usd', 100, 0, NULL, 0, 0)",
        )
        .bind(guid).bind(name).bind(acc_type)
        .execute(&pool).await.unwrap();
    }

    // Two transactions: opening balance + groceries purchase
    sqlx::query(
        "INSERT INTO transactions (guid, currency_guid, num, post_date, enter_date, description) VALUES ('tx_opening', 'cmdty_usd', '', '2024-01-01 00:00:00', '2024-01-01 00:00:00', 'Opening Balance')",
    ).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO transactions (guid, currency_guid, num, post_date, enter_date, description) VALUES ('tx_groc', 'cmdty_usd', '', '2024-02-03 00:00:00', '2024-02-03 09:00:00', 'Whole Foods')",
    ).execute(&pool).await.unwrap();

    // Splits (positive = debit, negative = credit in GnuCash convention)
    for (guid, tx_guid, account_guid, value_num) in [
        ("sp_open_a", "tx_opening", "acc_checking", 100000i64),
        ("sp_open_b", "tx_opening", "acc_opening", -100000),
        ("sp_groc_a", "tx_groc", "acc_checking", -5000),
        ("sp_groc_b", "tx_groc", "acc_groceries", 5000),
    ] {
        sqlx::query(
            "INSERT INTO splits (guid, tx_guid, account_guid, memo, action, reconcile_state, value_num, value_denom, quantity_num, quantity_denom) VALUES (?, ?, ?, '', '', 'n', ?, 100, ?, 100)",
        )
        .bind(guid).bind(tx_guid).bind(account_guid)
        .bind(value_num).bind(value_num)
        .execute(&pool).await.unwrap();
    }

    pool.close().await;
    path
}

#[tokio::test]
async fn end_to_end_happy_path() {
    // Set up an encrypted Tally DB.
    // create_encrypted_db already runs migrations internally.
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("tally.db");
    let pool = create_encrypted_db(&db_path, "pp", &[0u8; 16]).await.unwrap();
    let hh_id = new_ulid();
    sqlx::query("INSERT INTO households (id, name, timezone, created_at) VALUES (?, 'H', 'America/Chicago', 0)")
        .bind(&hh_id).execute(&pool).await.unwrap();

    // Build GnuCash fixture.
    let fixture_dir = tempdir().unwrap();
    let fixture_path = build_happy_fixture(fixture_dir.path()).await;

    // --- Read phase ---
    let preview = reader::preview(&fixture_path).await.unwrap();
    assert!(preview.non_usd_accounts.is_empty(), "happy book must be USD-only");
    assert_eq!(preview.account_count, 3);
    assert_eq!(preview.transaction_count, 2);
    let book = reader::read(&fixture_path).await.unwrap();

    // --- Map phase ---
    let plan = mapper::build_default_plan(hh_id.clone(), new_ulid(), &book, new_ulid).unwrap();
    assert!(mapper::find_duplicate_names(&plan).is_empty());
    assert_eq!(plan.account_mappings.len(), 3);
    assert_eq!(plan.transactions.len(), 2);

    // --- Commit phase ---
    let receipt = committer::commit(&pool, &plan, 100).await.unwrap();
    assert_eq!(receipt.accounts_created, 3);
    assert_eq!(receipt.transactions_committed, 2);
    assert_eq!(receipt.transactions_skipped, 0);

    // Sanity: the actual DB rows match the receipt.
    let (txn_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transactions WHERE household_id = ? AND source = 'import'"
    ).bind(&hh_id).fetch_one(&pool).await.unwrap();
    assert_eq!(txn_count, 2);

    // --- Reconcile phase ---
    let report = reconcile::reconcile(&pool, &plan, &book).await.unwrap();
    assert_eq!(report.total_mismatches, 0);
    assert_eq!(report.rows.len(), 3);
    for row in &report.rows {
        assert!(
            row.matches,
            "{} did not match: tally={} gnucash={}",
            row.account_name, row.tally_cents, row.gnucash_cents
        );
    }
}
