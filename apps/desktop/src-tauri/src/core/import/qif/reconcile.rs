//! Post-commit balance reconciliation. Compares each imported account's
//! Tally balance against the `B<balance>` declared in the QIF file. Returns a
//! [`QifBalanceReportArtifact`] for the UI to render.

use std::collections::HashMap;

use sqlx::SqlitePool;

use super::{QifBalanceReportArtifact, QifBalanceRow, QifBook, QifError, QifImportPlan};

pub async fn reconcile(
    pool: &SqlitePool,
    plan: &QifImportPlan,
    book: &QifBook,
) -> Result<QifBalanceReportArtifact, QifError> {
    let declared_by_name: HashMap<&str, i64> = book
        .accounts
        .iter()
        .filter_map(|a| a.declared_balance_cents.map(|c| (a.name.as_str(), c)))
        .collect();

    let mut rows: Vec<QifBalanceRow> = Vec::new();
    let mut mismatches: u32 = 0;
    for m in &plan.account_mappings {
        let Some(declared) = declared_by_name.get(m.qif_name.as_str()).copied() else {
            continue;
        };

        let (debits, credits): (Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT
                SUM(CASE WHEN jl.side = 'debit' THEN jl.amount ELSE 0 END),
                SUM(CASE WHEN jl.side = 'credit' THEN jl.amount ELSE 0 END)
             FROM journal_lines jl
             INNER JOIN transactions t ON t.id = jl.transaction_id
             WHERE jl.account_id = ? AND t.status = 'posted'",
        )
        .bind(&m.tally_account_id)
        .fetch_one(pool)
        .await?;

        let tally_cents = debits.unwrap_or(0) - credits.unwrap_or(0);
        let matches = tally_cents == declared;
        if !matches {
            mismatches += 1;
        }
        rows.push(QifBalanceRow {
            account_name: m.qif_name.clone(),
            tally_cents,
            declared_cents: declared,
            matches,
        });
    }

    Ok(QifBalanceReportArtifact {
        rows,
        total_mismatches: mismatches,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::import::qif::committer::commit;
    use crate::core::import::qif::mapper::build_default_plan;
    use crate::core::import::qif::reader;
    use crate::core::import::qif::test_fixtures;
    use crate::db::{connection::create_encrypted_db, migrations::run_migrations};
    use crate::id::new_ulid;
    use tempfile::tempdir;

    async fn setup_db() -> (tempfile::TempDir, sqlx::SqlitePool, String) {
        let dir = tempdir().unwrap();
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

    #[tokio::test]
    async fn fixture_reconciles_with_zero_mismatches() {
        let (_dir, pool, hh) = setup_db().await;
        let book = reader::parse(test_fixtures::banktivity_minimal()).unwrap();
        let plan = build_default_plan(hh.clone(), new_ulid(), &book, new_ulid).unwrap();
        commit(&pool, &plan, 0, 100).await.unwrap();

        let report = reconcile(&pool, &plan, &book).await.unwrap();
        assert_eq!(report.total_mismatches, 0, "got mismatches: {:#?}", report.rows);
        // Only the 3 real accounts have declared balances; synthesized ones
        // don't, so the report has 3 rows.
        assert_eq!(report.rows.len(), 3);
    }

    #[tokio::test]
    async fn manual_tampering_surfaces_as_mismatch() {
        let (_dir, pool, hh) = setup_db().await;
        let book = reader::parse(test_fixtures::banktivity_minimal()).unwrap();
        let plan = build_default_plan(hh.clone(), new_ulid(), &book, new_ulid).unwrap();
        commit(&pool, &plan, 0, 100).await.unwrap();

        // Tamper a real account: bump the amount of one journal line on
        // TestChecking by $100. That account has a declared balance, so the
        // reconciler must flag it.
        sqlx::query(
            "UPDATE journal_lines
             SET amount = amount + 10000
             WHERE id = (
                 SELECT jl.id FROM journal_lines jl
                 INNER JOIN accounts a ON a.id = jl.account_id
                 WHERE a.name = 'TestChecking'
                 LIMIT 1
             )",
        )
        .execute(&pool)
        .await
        .unwrap();

        let report = reconcile(&pool, &plan, &book).await.unwrap();
        assert!(report.total_mismatches > 0);
    }
}
