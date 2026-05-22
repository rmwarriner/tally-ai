//! End-to-end integration test for the QIF importer. Exercises the full
//! pipeline (reader → mapper → committer → reconciler) against a real
//! encrypted SQLite database with all migrations applied.

use sqlx::SqlitePool;
use std::path::PathBuf;
use tempfile::TempDir;

use tally_desktop_lib::core::import::qif::{committer, mapper, reader, reconcile};
use tally_desktop_lib::db::{create_encrypted_db, run_migrations};
use tally_desktop_lib::id::new_ulid;

/// Self-contained fixture mirroring the structure of a real Banktivity QIF
/// export. Multi-account, splits, transfers, opening balances.
const FIXTURE: &str = concat!(
    "!Option:AutoSwitch\n",
    "!Account\n",
    "NTestChecking\nTBank\nB1050.00\n^\n",
    "NTestCard\nTCCard\nB-80.00\n^\n",
    "NTestSavings\nTBank\nB600.00\n^\n",
    "!Clear:AutoSwitch\n",
    "!Type:Cat\n",
    "NNeeds:Groceries\nE\n^\n",
    "NEmployment Income:Salary\nI\n^\n",
    "!Account\n",
    "NTestChecking\nTBank\n^\n",
    "!Type:Bank\n",
    "D1/1/26\nT1000.00\nCc\nPSTARTING BALANCE\nMBALANCE ADJUSTMENT\n^\n",
    "D1/4/26\nT-50.00\nCc\nPGrocer\nLNeeds:Groceries\nMWeekly shop\n^\n",
    "D1/5/26\nT-100.00\nCc\nPTransfer\nL[TestSavings]\n^\n",
    "D1/10/26\nT200.00\nCc\nPEmployer\nLEmployment Income:Salary\n^\n",
    "!Account\n",
    "NTestCard\nTCCard\n^\n",
    "!Type:CCard\n",
    "D1/1/26\nT-50.00\nCc\nPSTARTING BALANCE\nMBALANCE ADJUSTMENT\n^\n",
    "D1/20/26\nT-30.00\nCc\nPAmazon\n",
    "EItemA\nSNeeds:Groceries\n$-20.00\n",
    "EItemB\nSNeeds:Groceries\n$-10.00\n",
    "^\n",
    "!Account\n",
    "NTestSavings\nTBank\n^\n",
    "!Type:Bank\n",
    "D1/1/26\nT500.00\nCc\nPSTARTING BALANCE\nMBALANCE ADJUSTMENT\n^\n",
    "D1/5/26\nT100.00\nCc\nPTransfer\nL[TestChecking]\n^\n",
);

async fn setup_db() -> (TempDir, SqlitePool, String) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("tally.db");
    let pool = create_encrypted_db(&path, "pp", &[0u8; 16]).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let hh = new_ulid();
    sqlx::query(
        "INSERT INTO households (id, name, timezone, created_at) VALUES (?, 'H', 'UTC', 0)",
    )
    .bind(&hh)
    .execute(&pool)
    .await
    .unwrap();
    (dir, pool, hh)
}

fn write_fixture(dir: &TempDir) -> PathBuf {
    let path = dir.path().join("fixture.qif");
    std::fs::write(&path, FIXTURE).unwrap();
    path
}

#[tokio::test]
async fn full_pipeline_end_to_end_reconciles_cleanly() {
    let (dir, pool, hh) = setup_db().await;
    let qif_path = write_fixture(&dir);

    // 1. Read
    let book = reader::read(&qif_path).await.unwrap();
    assert_eq!(book.accounts.len(), 3);
    assert!(!book.transactions.is_empty());

    // 2. Map
    let plan = mapper::build_default_plan(hh.clone(), new_ulid(), &book, new_ulid).unwrap();
    assert!(mapper::find_duplicate_names(&plan).is_empty());

    // 3. Commit
    let receipt = committer::commit(&pool, &plan, book.skipped_security_trades, 1000)
        .await
        .unwrap();
    assert_eq!(receipt.transactions_committed as usize, plan.transactions.len());
    assert_eq!(receipt.transactions_skipped, 0);

    // 4. Reconcile — every declared balance should match.
    let report = reconcile::reconcile(&pool, &plan, &book).await.unwrap();
    assert_eq!(
        report.total_mismatches, 0,
        "expected zero mismatches, got: {:#?}",
        report.rows
    );
    assert_eq!(report.rows.len(), 3, "one row per declared account");

    // 5. audit_log was written.
    let (audit_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM audit_log WHERE household_id = ? AND table_name = 'transactions'",
    )
    .bind(&hh)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit_count as usize, plan.transactions.len());
}

#[tokio::test]
async fn second_import_is_idempotent() {
    let (dir, pool, hh) = setup_db().await;
    let qif_path = write_fixture(&dir);

    let book = reader::read(&qif_path).await.unwrap();
    let plan_a = mapper::build_default_plan(hh.clone(), new_ulid(), &book, new_ulid).unwrap();
    committer::commit(&pool, &plan_a, 0, 1000).await.unwrap();

    // Second import with fresh account ULIDs but same source_refs.
    let plan_b = mapper::build_default_plan(hh.clone(), new_ulid(), &book, new_ulid).unwrap();
    let receipt = committer::commit(&pool, &plan_b, 0, 2000).await.unwrap();
    assert_eq!(receipt.transactions_committed, 0);
    assert_eq!(
        receipt.transactions_skipped as usize,
        plan_b.transactions.len()
    );
}

#[tokio::test]
async fn rollback_leaves_household_with_zero_imported_rows() {
    let (dir, pool, hh) = setup_db().await;
    let qif_path = write_fixture(&dir);

    let book = reader::read(&qif_path).await.unwrap();
    let plan = mapper::build_default_plan(hh.clone(), new_ulid(), &book, new_ulid).unwrap();
    committer::commit(&pool, &plan, 0, 1000).await.unwrap();

    committer::rollback(&pool, &plan.import_id).await.unwrap();

    let (acc,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE household_id = ?")
        .bind(&hh)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(acc, 0);

    let (txn,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE household_id = ?")
            .bind(&hh)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(txn, 0);
}
