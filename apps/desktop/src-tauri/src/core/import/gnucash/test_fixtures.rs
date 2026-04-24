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
