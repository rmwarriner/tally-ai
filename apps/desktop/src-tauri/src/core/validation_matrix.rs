//! T-060 — canonical inventory of validation behaviors.
//!
//! Every Tier 1 (HardError), Tier 2 (SoftWarning), and Tier 3 (AIAdvisory)
//! variant has at least one positive-trigger test, one non-trigger test, and
//! (where meaningful) one boundary/edge test. Each test asserts the expected
//! recovery action set against the spec, not just the error variant.

#![cfg(test)]

use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::SqlitePool;
use tempfile::tempdir;

use crate::core::coa::seed_chart_of_accounts;
use crate::core::envelope::create_envelope_with_current_period;
use crate::core::proposal::{ProposedLine, Side, TransactionProposal};
use crate::core::validation::{
    validate_proposal, AIAdvisory, HardError, HardErrorCode, SoftWarning, SoftWarningCode,
    ValidationResult,
};
use crate::db::{connection::create_encrypted_db, migrations::run_migrations};
use crate::error::RecoveryKind;
use crate::id::new_ulid;

// Shared fixture helpers ---------------------------------------------------

const ONE_DAY_MS: i64 = 86_400_000;

/// Spins up a fresh encrypted SQLite pool with all migrations applied.
/// Mirrors `validation.rs::tests::test_pool` — duplicated inline to keep the
/// matrix module self-contained.
async fn fresh_pool() -> SqlitePool {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.keep().join("matrix.db");
    let pool = create_encrypted_db(&db_path, "test", &[0u8; 16])
        .await
        .expect("create db");
    run_migrations(&pool).await.expect("migrate");
    pool
}

/// IDs returned by [`seed_household`]. The expense account is the canonical
/// "Groceries" leaf (debit-normal); the cash account is "Cash" (debit-normal,
/// pre-funded with $100 so the baseline credit doesn't trigger AbnormalBalance).
/// The grocery envelope is always seeded — tests that don't need it can ignore it.
struct SeedIds {
    household_id: String,
    cash_account_id: String,
    expense_account_id: String,
    grocery_envelope_id: String,
}

/// Seeds a household with the standard chart of accounts, a $100 opening
/// balance on Cash (via Opening Balance Equity), and a "Groceries" envelope.
async fn seed_household(pool: &SqlitePool) -> SeedIds {
    let household_id = new_ulid();
    sqlx::query(
        "INSERT INTO households (id, name, timezone, created_at) VALUES (?, 'Test', 'UTC', 0)",
    )
    .bind(&household_id)
    .execute(pool)
    .await
    .expect("insert household");

    seed_chart_of_accounts(pool, &household_id)
        .await
        .expect("seed CoA");

    let (cash_account_id,): (String,) = sqlx::query_as(
        "SELECT id FROM accounts WHERE household_id = ? AND name = 'Cash' AND is_placeholder = 0",
    )
    .bind(&household_id)
    .fetch_one(pool)
    .await
    .expect("look up Cash");

    let (expense_account_id,): (String,) = sqlx::query_as(
        "SELECT id FROM accounts WHERE household_id = ? AND name = 'Groceries' AND is_placeholder = 0",
    )
    .bind(&household_id)
    .fetch_one(pool)
    .await
    .expect("look up Groceries");

    let (obe_account_id,): (String,) = sqlx::query_as(
        "SELECT id FROM accounts WHERE household_id = ? AND name = 'Opening Balance Equity' AND is_placeholder = 0",
    )
    .bind(&household_id)
    .fetch_one(pool)
    .await
    .expect("look up OBE");

    // Seed a $100 opening balance on Cash via a posted opening-equity txn.
    // Without this, the baseline (credit Cash $15) would push Cash to -1500
    // cents and trip AbnormalBalance.
    let opening_txn_id = new_ulid();
    sqlx::query(
        "INSERT INTO transactions
             (id, household_id, txn_date, entry_date, status, source, created_at)
         VALUES (?, ?, 0, 0, 'posted', 'manual', 0)",
    )
    .bind(&opening_txn_id)
    .bind(&household_id)
    .execute(pool)
    .await
    .expect("seed opening txn");

    sqlx::query(
        "INSERT INTO journal_lines
             (id, transaction_id, account_id, amount, side, created_at)
         VALUES (?, ?, ?, 10000, 'debit', 0)",
    )
    .bind(new_ulid())
    .bind(&opening_txn_id)
    .bind(&cash_account_id)
    .execute(pool)
    .await
    .expect("seed cash debit");

    sqlx::query(
        "INSERT INTO journal_lines
             (id, transaction_id, account_id, amount, side, created_at)
         VALUES (?, ?, ?, 10000, 'credit', 0)",
    )
    .bind(new_ulid())
    .bind(&opening_txn_id)
    .bind(&obe_account_id)
    .execute(pool)
    .await
    .expect("seed OBE credit");

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let grocery_envelope_id =
        create_envelope_with_current_period(pool, &household_id, "Groceries", now_ms)
            .await
            .expect("create grocery envelope");

    SeedIds {
        household_id,
        cash_account_id,
        expense_account_id,
        grocery_envelope_id,
    }
}

/// A clean two-line proposal: debit Groceries $15, credit Cash $15, dated
/// seven days ago. Picked so the baseline trips no Tier 1/2 rule. Tasks 4–6
/// mutate this baseline to exercise individual rules.
fn baseline_proposal_for(seed: &SeedIds) -> TransactionProposal {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    TransactionProposal {
        memo: Some("Baseline grocery run".to_string()),
        txn_date_ms: now_ms - 7 * ONE_DAY_MS,
        lines: vec![
            ProposedLine {
                account_id: seed.expense_account_id.clone(),
                envelope_id: None,
                amount_cents: 1500,
                side: Side::Debit,
            },
            ProposedLine {
                account_id: seed.cash_account_id.clone(),
                envelope_id: None,
                amount_cents: 1500,
                side: Side::Credit,
            },
        ],
    }
}

fn hard_codes(result: &ValidationResult) -> Vec<HardErrorCode> {
    match result {
        ValidationResult::Rejected { errors, .. } => {
            errors.iter().map(|e| e.code).collect()
        }
        _ => vec![],
    }
}

fn soft_codes(result: &ValidationResult) -> Vec<SoftWarningCode> {
    match result {
        ValidationResult::Warnings { warnings, .. } => {
            warnings.iter().map(|w| w.code).collect()
        }
        ValidationResult::Rejected { warnings, .. } => {
            warnings.iter().map(|w| w.code).collect()
        }
        _ => vec![],
    }
}

fn recovery_kinds_of_hard(err: &HardError) -> Vec<RecoveryKind> {
    err.actions.iter().map(|a| a.kind.clone()).collect()
}

fn recovery_kinds_of_soft(warn: &SoftWarning) -> Vec<RecoveryKind> {
    warn.actions.iter().map(|a| a.kind.clone()).collect()
}

fn recovery_kinds_of_advisory(adv: &AIAdvisory) -> Vec<RecoveryKind> {
    adv.actions.iter().map(|a| a.kind.clone()).collect()
}

// Sanity check ------------------------------------------------------------

/// Smoke test: the seeded baseline proposal must validate cleanly. If this
/// ever flips, every Task 4/5/6 matrix test built on top of `seed_household`
/// + `baseline_proposal_for` is suspect.
#[tokio::test]
async fn matrix_baseline_validates_clean() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let proposal = baseline_proposal_for(&seed);
    let result = validate_proposal(&pool, &proposal).await;
    assert!(
        matches!(result, ValidationResult::Accepted),
        "baseline must be accepted, got {result:?}"
    );
}
