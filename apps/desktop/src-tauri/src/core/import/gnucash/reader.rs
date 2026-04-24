//! Opens a GnuCash SQLite file read-only and builds a `GnuCashBook`.
//!
//! GnuCash stores amounts as signed `value_num / value_denom` rationals.
//! Our reader normalizes every split to signed cents (denom=100 for USD).

use super::{
    GnuCashBook, GncAccount, GncAccountType, GncCommodity, GncSplit, GncTransaction, GnuCashPreview,
    ImportError,
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

    // A commodity is USD iff namespace='CURRENCY' AND mnemonic='USD'.
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

fn normalize_to_cents(num: i64, denom: i64) -> i64 {
    if denom == 100 || denom == 0 {
        return num;
    }
    (num * 100) / denom
}

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
        let dir = tempdir().unwrap();
        let mut spec = happy_spec();
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

    #[tokio::test]
    async fn non_usd_account_flagged_in_preview() {
        let dir = tempdir().unwrap();
        let mut spec = happy_spec();
        spec.book_guid = "book_eur".into();
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

        let preview_result = preview(&path).await.expect("preview should still succeed");
        assert!(preview_result.non_usd_accounts.contains(&"Euro Savings".to_string()));
        assert_eq!(preview_result.account_count, 4);
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

        let preview_result = preview(&path).await.unwrap();
        assert!(preview_result.non_usd_accounts.contains(&"AAPL".to_string()));
    }

    #[tokio::test]
    async fn happy_preview_has_empty_non_usd_list() {
        let dir = tempdir().unwrap();
        let path = build_fixture(dir.path(), &happy_spec()).await;
        let p = preview(&path).await.unwrap();
        assert!(p.non_usd_accounts.is_empty());
        assert_eq!(p.transaction_count, 2);
        assert_eq!(p.account_count, 3);
    }

    #[tokio::test]
    async fn empty_sqlite_rejected_as_not_gnucash() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("empty.sqlite");
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
}
