//! After commit, compute per-account expected balances from the source book
//! and compare them against Tally's current balances. Produces a
//! `BalanceReport` payload for the existing frontend renderer.

use super::{GnuCashBook, ImportError, ImportPlan};
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

pub async fn reconcile(
    pool: &SqlitePool,
    plan: &ImportPlan,
    book: &GnuCashBook,
) -> Result<BalanceReportArtifact, ImportError> {
    let expected = expected_balances_by_guid(book);

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
    async fn reconcile_handles_duplicate_leaf_names() {
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

        // Build a fixture with two "Savings" accounts under different parents.
        let fixture_dir = tempdir().unwrap();
        let mut spec = happy_spec();
        spec.book_guid = "book_dup_leaf".into();
        spec.accounts.push(super::super::test_fixtures::FixtureAccount {
            guid: "acc_assets_parent".into(),
            name: "Assets".into(),
            account_type: "ASSET",
            commodity_guid: "cmdty_usd".into(),
            parent_guid: None,
            placeholder: true,
            hidden: false,
        });
        spec.accounts.push(super::super::test_fixtures::FixtureAccount {
            guid: "acc_invest_parent".into(),
            name: "Investments".into(),
            account_type: "ASSET",
            commodity_guid: "cmdty_usd".into(),
            parent_guid: None,
            placeholder: true,
            hidden: false,
        });
        spec.accounts.push(super::super::test_fixtures::FixtureAccount {
            guid: "acc_savings_a".into(),
            name: "Savings".into(),
            account_type: "BANK",
            commodity_guid: "cmdty_usd".into(),
            parent_guid: Some("acc_assets_parent".into()),
            placeholder: false,
            hidden: false,
        });
        spec.accounts.push(super::super::test_fixtures::FixtureAccount {
            guid: "acc_savings_b".into(),
            name: "Savings".into(),
            account_type: "BANK",
            commodity_guid: "cmdty_usd".into(),
            parent_guid: Some("acc_invest_parent".into()),
            placeholder: false,
            hidden: false,
        });
        let fixture_path = build_fixture(fixture_dir.path(), &spec).await;
        let book = read(&fixture_path).await.unwrap();
        let plan = build_default_plan(hh_id.clone(), crate::id::new_ulid(), &book, crate::id::new_ulid).unwrap();
        commit(&pool, &plan, 100).await.unwrap();

        let report = reconcile(&pool, &plan, &book).await.unwrap();
        // Both Savings rows must appear (by full_name) with matches=true.
        let assets_savings = report.rows.iter().find(|r| r.account_name == "Assets:Savings")
            .expect("Assets:Savings row present");
        let invest_savings = report.rows.iter().find(|r| r.account_name == "Investments:Savings")
            .expect("Investments:Savings row present");
        assert!(assets_savings.matches);
        assert!(invest_savings.matches);
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

        // Corrupt one journal line's amount after the fact.
        // LIMIT inside UPDATE may not be enabled in this SQLite build — use a WHERE that picks one row.
        sqlx::query(
            "UPDATE journal_lines SET amount = amount + 100 \
             WHERE id = (SELECT jl.id FROM journal_lines jl \
                         JOIN transactions t ON t.id = jl.transaction_id \
                         WHERE t.source_ref = 'tx_groc' LIMIT 1)"
        )
        .execute(&pool).await.unwrap();

        let report = reconcile(&pool, &plan, &book).await.unwrap();
        assert!(report.total_mismatches > 0);
        assert!(report.rows.iter().any(|r| !r.matches));
    }
}
