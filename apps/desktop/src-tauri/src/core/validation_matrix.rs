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

use crate::ai::advisories;
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

// === Task 4: Tier 1 (HardError) matrix ====================================
//
// Each variant has at least:
//   - `tier1_<variant>_triggers` — mutate baseline to trip the rule.
//   - `tier1_<variant>_does_not_trigger_when_clean` — clean baseline.
//   - `tier1_<variant>_edge_<scenario>` — boundary case where meaningful;
//     skipped with a comment otherwise.
//
// Recovery action expectations are verified against `validation.rs`
// (`hard_error(..., primary_action, vec![extras...])`) — *not* the design
// table — so the test reflects what the validator actually emits.

fn find_hard(
    result: &ValidationResult,
    code: HardErrorCode,
) -> Option<&HardError> {
    match result {
        ValidationResult::Rejected { errors, .. } => {
            errors.iter().find(|e| e.code == code)
        }
        _ => None,
    }
}

fn first_kind_is(kinds: &[RecoveryKind], expected: RecoveryKind) -> bool {
    kinds.first() == Some(&expected)
}

fn kinds_contain(kinds: &[RecoveryKind], expected: RecoveryKind) -> bool {
    kinds.contains(&expected)
}

// --- NoLines --------------------------------------------------------------

#[tokio::test]
async fn tier1_no_lines_triggers() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let mut p = baseline_proposal_for(&seed);
    p.lines.clear();

    let result = validate_proposal(&pool, &p).await;

    assert!(
        hard_codes(&result).contains(&HardErrorCode::NoLines),
        "expected NoLines but got: {:?}",
        hard_codes(&result),
    );
    let err = find_hard(&result, HardErrorCode::NoLines).expect("NoLines error present");
    let kinds = recovery_kinds_of_hard(err);
    assert!(
        first_kind_is(&kinds, RecoveryKind::EditField),
        "primary recovery for NoLines: expected EditField, got {:?}",
        kinds.first(),
    );
    assert!(
        kinds_contain(&kinds, RecoveryKind::Discard),
        "NoLines should also offer Discard, got {:?}",
        kinds
    );
}

#[tokio::test]
async fn tier1_no_lines_does_not_trigger_when_clean() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let p = baseline_proposal_for(&seed);
    let result = validate_proposal(&pool, &p).await;
    assert!(!hard_codes(&result).contains(&HardErrorCode::NoLines));
}

// no edge case applicable — NoLines is binary (have ≥2 lines or not)

// --- UnbalancedLines ------------------------------------------------------

#[tokio::test]
async fn tier1_unbalanced_lines_triggers() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let mut p = baseline_proposal_for(&seed);
    p.lines[0].amount_cents = 1500;
    p.lines[1].amount_cents = 1499;

    let result = validate_proposal(&pool, &p).await;

    assert!(
        hard_codes(&result).contains(&HardErrorCode::UnbalancedLines),
        "expected UnbalancedLines but got: {:?}",
        hard_codes(&result),
    );
    let err = find_hard(&result, HardErrorCode::UnbalancedLines)
        .expect("UnbalancedLines error present");
    let kinds = recovery_kinds_of_hard(err);
    assert!(
        first_kind_is(&kinds, RecoveryKind::EditField),
        "primary recovery for UnbalancedLines: expected EditField, got {:?}",
        kinds.first(),
    );
    assert!(kinds_contain(&kinds, RecoveryKind::Discard));
}

#[tokio::test]
async fn tier1_unbalanced_lines_does_not_trigger_when_clean() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let p = baseline_proposal_for(&seed);
    let result = validate_proposal(&pool, &p).await;
    assert!(!hard_codes(&result).contains(&HardErrorCode::UnbalancedLines));
}

// no edge case applicable — debit_sum != credit_sum is binary; "off by one
// cent" is already the trigger case.

// --- ZeroAmount -----------------------------------------------------------

#[tokio::test]
async fn tier1_zero_amount_triggers() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let mut p = baseline_proposal_for(&seed);
    p.lines[0].amount_cents = 0;
    p.lines[1].amount_cents = 0;

    let result = validate_proposal(&pool, &p).await;

    assert!(
        hard_codes(&result).contains(&HardErrorCode::ZeroAmount),
        "expected ZeroAmount but got: {:?}",
        hard_codes(&result),
    );
    let err = find_hard(&result, HardErrorCode::ZeroAmount).expect("ZeroAmount present");
    let kinds = recovery_kinds_of_hard(err);
    assert!(first_kind_is(&kinds, RecoveryKind::EditField));
    assert!(kinds_contain(&kinds, RecoveryKind::Discard));
}

#[tokio::test]
async fn tier1_zero_amount_does_not_trigger_when_clean() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let p = baseline_proposal_for(&seed);
    let result = validate_proposal(&pool, &p).await;
    assert!(!hard_codes(&result).contains(&HardErrorCode::ZeroAmount));
}

#[tokio::test]
async fn tier1_zero_amount_edge_one_cent_passes() {
    // Boundary: amount_cents = 1 must NOT trigger ZeroAmount. The rule is
    // strictly `== 0`. Both lines set to 1 cent so the proposal stays balanced.
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let mut p = baseline_proposal_for(&seed);
    p.lines[0].amount_cents = 1;
    p.lines[1].amount_cents = 1;

    let result = validate_proposal(&pool, &p).await;
    assert!(
        !hard_codes(&result).contains(&HardErrorCode::ZeroAmount),
        "1 cent should pass the zero check, got {:?}",
        hard_codes(&result),
    );
}

// --- NegativeAmount -------------------------------------------------------

#[tokio::test]
async fn tier1_negative_amount_triggers() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let mut p = baseline_proposal_for(&seed);
    // Both lines flipped negative so the proposal is still balanced;
    // we want NegativeAmount to fire on its own (UnbalancedLines doesn't piggyback).
    p.lines[0].amount_cents = -1500;
    p.lines[1].amount_cents = -1500;

    let result = validate_proposal(&pool, &p).await;

    assert!(
        hard_codes(&result).contains(&HardErrorCode::NegativeAmount),
        "expected NegativeAmount but got: {:?}",
        hard_codes(&result),
    );
    let err = find_hard(&result, HardErrorCode::NegativeAmount).expect("NegativeAmount present");
    let kinds = recovery_kinds_of_hard(err);
    assert!(first_kind_is(&kinds, RecoveryKind::EditField));
    assert!(kinds_contain(&kinds, RecoveryKind::Discard));
}

#[tokio::test]
async fn tier1_negative_amount_does_not_trigger_when_clean() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let p = baseline_proposal_for(&seed);
    let result = validate_proposal(&pool, &p).await;
    assert!(!hard_codes(&result).contains(&HardErrorCode::NegativeAmount));
}

// no edge case applicable — `< 0` is a strict sign check; 0 is covered by ZeroAmount.

// --- UnknownAccount -------------------------------------------------------

#[tokio::test]
async fn tier1_unknown_account_triggers() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let mut p = baseline_proposal_for(&seed);
    // A well-formed ULID that isn't in the seeded CoA.
    p.lines[0].account_id = "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string();

    let result = validate_proposal(&pool, &p).await;

    assert!(
        hard_codes(&result).contains(&HardErrorCode::UnknownAccount),
        "expected UnknownAccount but got: {:?}",
        hard_codes(&result),
    );
    let err = find_hard(&result, HardErrorCode::UnknownAccount).expect("UnknownAccount present");
    let kinds = recovery_kinds_of_hard(err);
    // validation.rs emits `CreateMissing` (primary) + `Discard` (extras only).
    // The original task table listed `EditField` too, but the validator does
    // not produce one — assert the actual set.
    assert!(
        first_kind_is(&kinds, RecoveryKind::CreateMissing),
        "primary recovery for UnknownAccount: expected CreateMissing, got {:?}",
        kinds.first(),
    );
    assert!(kinds_contain(&kinds, RecoveryKind::Discard));
}

#[tokio::test]
async fn tier1_unknown_account_does_not_trigger_when_clean() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let p = baseline_proposal_for(&seed);
    let result = validate_proposal(&pool, &p).await;
    assert!(!hard_codes(&result).contains(&HardErrorCode::UnknownAccount));
}

// no edge case applicable — account either exists in `accounts` or it doesn't.

// --- PlaceholderAccount ---------------------------------------------------

#[tokio::test]
async fn tier1_placeholder_account_triggers() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let mut p = baseline_proposal_for(&seed);

    // Pull any placeholder account from the seeded CoA. The standard CoA seed
    // creates the five top-level groupings (Assets, Liabilities, Equity,
    // Income, Expenses) as placeholders.
    let placeholder_id: String = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE household_id = ? AND is_placeholder = 1 LIMIT 1",
    )
    .bind(&seed.household_id)
    .fetch_one(&pool)
    .await
    .expect("a placeholder account exists in the CoA seed");
    p.lines[0].account_id = placeholder_id;

    let result = validate_proposal(&pool, &p).await;

    assert!(
        hard_codes(&result).contains(&HardErrorCode::PlaceholderAccount),
        "expected PlaceholderAccount but got: {:?}",
        hard_codes(&result),
    );
    let err = find_hard(&result, HardErrorCode::PlaceholderAccount)
        .expect("PlaceholderAccount present");
    let kinds = recovery_kinds_of_hard(err);
    assert!(first_kind_is(&kinds, RecoveryKind::EditField));
    assert!(kinds_contain(&kinds, RecoveryKind::Discard));
}

#[tokio::test]
async fn tier1_placeholder_account_does_not_trigger_when_clean() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let p = baseline_proposal_for(&seed);
    let result = validate_proposal(&pool, &p).await;
    assert!(!hard_codes(&result).contains(&HardErrorCode::PlaceholderAccount));
}

// no edge case applicable — `is_placeholder` is a boolean column.

// --- AbnormalBalance ------------------------------------------------------

#[tokio::test]
async fn tier1_abnormal_balance_triggers() {
    // Mirrors `validate_rejects_abnormal_balance` in validation.rs: credit a
    // debit-normal account (Cash) by more than its standing balance so the
    // resulting balance would go negative-from-normal.
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let mut p = baseline_proposal_for(&seed);
    // Cash has a $100 opening debit balance from seeding. Credit $200 against
    // it — Cash would end at -$100 (abnormal for a debit-normal account).
    p.lines[0].amount_cents = 20_000; // expense debit
    p.lines[1].amount_cents = 20_000; // cash credit

    let result = validate_proposal(&pool, &p).await;

    assert!(
        hard_codes(&result).contains(&HardErrorCode::AbnormalBalance),
        "expected AbnormalBalance but got: {:?}",
        hard_codes(&result),
    );
    let err =
        find_hard(&result, HardErrorCode::AbnormalBalance).expect("AbnormalBalance present");
    let kinds = recovery_kinds_of_hard(err);
    assert!(first_kind_is(&kinds, RecoveryKind::EditField));
    assert!(kinds_contain(&kinds, RecoveryKind::PostAnyway));
    assert!(kinds_contain(&kinds, RecoveryKind::Discard));
}

#[tokio::test]
async fn tier1_abnormal_balance_does_not_trigger_when_clean() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let p = baseline_proposal_for(&seed);
    let result = validate_proposal(&pool, &p).await;
    assert!(!hard_codes(&result).contains(&HardErrorCode::AbnormalBalance));
}

#[tokio::test]
async fn tier1_abnormal_balance_edge_exact_zero_swing_passes() {
    // Boundary: drain Cash to exactly $0. The rule fires on `current + net < 0`,
    // not `<= 0`, so a balance landing on zero is allowed.
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let mut p = baseline_proposal_for(&seed);
    p.lines[0].amount_cents = 10_000; // expense debit $100
    p.lines[1].amount_cents = 10_000; // cash credit $100 → Cash 100 - 100 = 0

    let result = validate_proposal(&pool, &p).await;
    assert!(
        !hard_codes(&result).contains(&HardErrorCode::AbnormalBalance),
        "$0 swing should pass the abnormal-balance check, got {:?}",
        hard_codes(&result),
    );
}

// --- EnvelopeMismatch -----------------------------------------------------
//
// NOTE: As of 2026-04-26, `HardErrorCode::EnvelopeMismatch` is declared in
// the enum but never emitted anywhere in `validation.rs`. The intended rule
// — "envelope_id only valid on expense lines" — is not implemented. We keep
// the matrix slot so the inventory is complete: the trigger test documents
// today's behavior (no rule fires) and will fail loudly the day the rule
// lands, prompting a real assertion.

#[tokio::test]
async fn tier1_envelope_mismatch_triggers() {
    // Attach an envelope to the cash credit line — per the design rule this
    // should reject (envelopes are expense-only). Today the validator is
    // silent; assert the *current* behavior so regressions are visible.
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let mut p = baseline_proposal_for(&seed);
    p.lines[1].envelope_id = Some(seed.grocery_envelope_id.clone());

    let result = validate_proposal(&pool, &p).await;

    // TODO(t-060): once EnvelopeMismatch is implemented, flip this to
    // `assert!(hard_codes(&result).contains(&HardErrorCode::EnvelopeMismatch))`
    // and assert the recovery-kind set against the new code.
    assert!(
        !hard_codes(&result).contains(&HardErrorCode::EnvelopeMismatch),
        "EnvelopeMismatch is not yet implemented in validation.rs — \
         if this test starts failing, the rule landed: update the matrix \
         to assert the intended behavior. Got: {:?}",
        hard_codes(&result),
    );
}

#[tokio::test]
async fn tier1_envelope_mismatch_does_not_trigger_when_clean() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let p = baseline_proposal_for(&seed);
    let result = validate_proposal(&pool, &p).await;
    assert!(!hard_codes(&result).contains(&HardErrorCode::EnvelopeMismatch));
}

// no edge case applicable — rule is unimplemented; once it lands, an edge
// (e.g. envelope on an income line, or envelope on a debit-side expense)
// should be added.

// === Task 5: Tier 2 (SoftWarning) matrix ==================================
//
// Each variant has at least:
//   - `tier2_<variant>_triggers` — mutate baseline to trip the warning.
//   - `tier2_<variant>_does_not_trigger_when_clean` — clean baseline.
//   - `tier2_<variant>_edge_<scenario>` — boundary case where meaningful.
//
// Recovery action expectations are verified against `validation.rs`
// (`soft_warning(..., primary_action, vec![extras...])`) — *not* the design
// table — so the test reflects what the validator actually emits.
//
// Discoveries vs. the original design table for Tier 2:
//   - FutureDate / StaleDate / LargeAmount: validator emits `EditField`
//     primary + `PostAnyway` extra. NO `Discard`. Table claimed primary
//     `PostAnyway`; that was wrong.
//   - EnvelopeOverdraft: validator emits `PostAnyway` primary +
//     `EditField` ("Change envelope") extra. NO `Discard`. Table claimed
//     a `Discard` extra; that was wrong.
//   - PossibleDuplicate: matches the table — `PostAnyway` primary +
//     `Discard` extra.

/// Look up the credit-card account on a seeded household. Used as a
/// credit-normal counter-account when the test needs to push a large amount
/// without tripping `AbnormalBalance` on Cash. (Cash only has $100 of opening
/// balance — $500.01 against it goes negative.)
async fn credit_card_account_id(pool: &SqlitePool, household_id: &str) -> String {
    let (id,): (String,) = sqlx::query_as(
        "SELECT id FROM accounts WHERE household_id = ? AND name = 'Credit Card' AND is_placeholder = 0",
    )
    .bind(household_id)
    .fetch_one(pool)
    .await
    .expect("look up Credit Card");
    id
}

/// Allocates `cents` to the seeded grocery envelope's current period so
/// overdraft-edge tests can pin the boundary precisely. The default seed
/// allocates 0, which makes "exactly at cap" indistinguishable from the
/// trigger case.
async fn set_grocery_allocation(pool: &SqlitePool, envelope_id: &str, cents: i64) {
    sqlx::query(
        "UPDATE envelope_periods SET allocated = ? WHERE envelope_id = ?",
    )
    .bind(cents)
    .bind(envelope_id)
    .execute(pool)
    .await
    .expect("update allocation");
}

/// Inserts a posted transaction with a single debit + single credit line so
/// the duplicate detector (which matches by `SUM(debit) GROUP BY t.id`)
/// finds it. Mirrors `validation.rs::tests::soft_warn_possible_duplicate`.
async fn seed_posted_txn(
    pool: &SqlitePool,
    household_id: &str,
    debit_account_id: &str,
    credit_account_id: &str,
    amount_cents: i64,
    txn_date_ms: i64,
) {
    let txn_id = new_ulid();
    sqlx::query(
        "INSERT INTO transactions
             (id, household_id, txn_date, entry_date, status, source, created_at)
         VALUES (?, ?, ?, 0, 'posted', 'manual', 0)",
    )
    .bind(&txn_id)
    .bind(household_id)
    .bind(txn_date_ms)
    .execute(pool)
    .await
    .expect("seed posted txn");

    sqlx::query(
        "INSERT INTO journal_lines
             (id, transaction_id, account_id, amount, side, created_at)
         VALUES (?, ?, ?, ?, 'debit', 0)",
    )
    .bind(new_ulid())
    .bind(&txn_id)
    .bind(debit_account_id)
    .bind(amount_cents)
    .execute(pool)
    .await
    .expect("seed posted debit");

    sqlx::query(
        "INSERT INTO journal_lines
             (id, transaction_id, account_id, amount, side, created_at)
         VALUES (?, ?, ?, ?, 'credit', 0)",
    )
    .bind(new_ulid())
    .bind(&txn_id)
    .bind(credit_account_id)
    .bind(amount_cents)
    .execute(pool)
    .await
    .expect("seed posted credit");
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn find_soft(
    result: &ValidationResult,
    code: SoftWarningCode,
) -> Option<&SoftWarning> {
    match result {
        ValidationResult::Warnings { warnings, .. } => {
            warnings.iter().find(|w| w.code == code)
        }
        ValidationResult::Rejected { warnings, .. } => {
            warnings.iter().find(|w| w.code == code)
        }
        _ => None,
    }
}

// --- FutureDate -----------------------------------------------------------
//
// Threshold: `txn_date_ms > now_ms + ONE_DAY_MS` (strictly greater).
// Recovery: `EditField` primary, `PostAnyway` extra. No `Discard`.

#[tokio::test]
async fn tier2_future_date_triggers() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let mut p = baseline_proposal_for(&seed);
    p.txn_date_ms = now_ms() + 90 * ONE_DAY_MS;

    let result = validate_proposal(&pool, &p).await;

    assert!(
        soft_codes(&result).contains(&SoftWarningCode::FutureDate),
        "expected FutureDate but got: {:?}",
        soft_codes(&result),
    );
    let warn = find_soft(&result, SoftWarningCode::FutureDate)
        .expect("FutureDate warn present");
    let kinds = recovery_kinds_of_soft(warn);
    assert!(
        first_kind_is(&kinds, RecoveryKind::EditField),
        "primary recovery for FutureDate: expected EditField, got {:?}",
        kinds.first(),
    );
    assert!(
        kinds_contain(&kinds, RecoveryKind::PostAnyway),
        "FutureDate should also offer PostAnyway, got {:?}",
        kinds,
    );
}

#[tokio::test]
async fn tier2_future_date_does_not_trigger_when_clean() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let p = baseline_proposal_for(&seed);
    let result = validate_proposal(&pool, &p).await;
    assert!(!soft_codes(&result).contains(&SoftWarningCode::FutureDate));
}

#[tokio::test]
async fn tier2_future_date_edge_exactly_one_day_passes() {
    // Boundary: `> now + ONE_DAY_MS` is strict. A txn dated exactly one day
    // ahead must NOT trigger. (Use a small back-off below `now` to avoid
    // racing the clock between the test computing `now` and the validator
    // computing its own `now_ms`.)
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let mut p = baseline_proposal_for(&seed);
    // Pick a value that is at-or-below `validator_now + ONE_DAY_MS` even if a
    // few ms elapse between this line and the validator's clock read. Going
    // ~5 seconds under the boundary keeps us safely on the "no warn" side.
    p.txn_date_ms = now_ms() + ONE_DAY_MS - 5_000;

    let result = validate_proposal(&pool, &p).await;
    assert!(
        !soft_codes(&result).contains(&SoftWarningCode::FutureDate),
        "txn dated at-or-just-under +1 day should pass FutureDate, got {:?}",
        soft_codes(&result),
    );
}

// --- StaleDate ------------------------------------------------------------
//
// Threshold: `txn_date_ms < now_ms - NINETY_DAYS_MS` (strictly less).
// Recovery: `EditField` primary, `PostAnyway` extra. No `Discard`.

#[tokio::test]
async fn tier2_stale_date_triggers() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let mut p = baseline_proposal_for(&seed);
    p.txn_date_ms = now_ms() - 400 * ONE_DAY_MS;

    let result = validate_proposal(&pool, &p).await;

    assert!(
        soft_codes(&result).contains(&SoftWarningCode::StaleDate),
        "expected StaleDate but got: {:?}",
        soft_codes(&result),
    );
    let warn = find_soft(&result, SoftWarningCode::StaleDate)
        .expect("StaleDate warn present");
    let kinds = recovery_kinds_of_soft(warn);
    assert!(
        first_kind_is(&kinds, RecoveryKind::EditField),
        "primary recovery for StaleDate: expected EditField, got {:?}",
        kinds.first(),
    );
    assert!(
        kinds_contain(&kinds, RecoveryKind::PostAnyway),
        "StaleDate should also offer PostAnyway, got {:?}",
        kinds,
    );
}

#[tokio::test]
async fn tier2_stale_date_does_not_trigger_when_clean() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let p = baseline_proposal_for(&seed);
    let result = validate_proposal(&pool, &p).await;
    assert!(!soft_codes(&result).contains(&SoftWarningCode::StaleDate));
}

#[tokio::test]
async fn tier2_stale_date_edge_exactly_ninety_days_passes() {
    // Boundary: `< now - 90 days` is strict. Exactly 90 days back must NOT
    // trigger. We pad a few seconds under the boundary to absorb the clock
    // drift between this line and the validator's clock read.
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let mut p = baseline_proposal_for(&seed);
    p.txn_date_ms = now_ms() - 90 * ONE_DAY_MS + 5_000;

    let result = validate_proposal(&pool, &p).await;
    assert!(
        !soft_codes(&result).contains(&SoftWarningCode::StaleDate),
        "txn dated at-or-just-under 90 days back should pass StaleDate, got {:?}",
        soft_codes(&result),
    );
}

// --- LargeAmount ----------------------------------------------------------
//
// Threshold: any line `amount_cents > LARGE_AMOUNT_CENTS` (= 50_000, $500).
// Recovery: `EditField` primary, `PostAnyway` extra. No `Discard`.

#[tokio::test]
async fn tier2_large_amount_triggers() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let mut p = baseline_proposal_for(&seed);
    // Use Credit Card (credit-normal liability) instead of Cash so a $500.01
    // credit doesn't push Cash into AbnormalBalance and suppress the warning.
    let cc_id = credit_card_account_id(&pool, &seed.household_id).await;
    p.lines[1].account_id = cc_id;
    p.lines[0].amount_cents = 50_001;
    p.lines[1].amount_cents = 50_001;

    let result = validate_proposal(&pool, &p).await;

    assert!(
        soft_codes(&result).contains(&SoftWarningCode::LargeAmount),
        "expected LargeAmount but got: {:?}",
        soft_codes(&result),
    );
    let warn = find_soft(&result, SoftWarningCode::LargeAmount)
        .expect("LargeAmount warn present");
    let kinds = recovery_kinds_of_soft(warn);
    assert!(
        first_kind_is(&kinds, RecoveryKind::EditField),
        "primary recovery for LargeAmount: expected EditField, got {:?}",
        kinds.first(),
    );
    assert!(
        kinds_contain(&kinds, RecoveryKind::PostAnyway),
        "LargeAmount should also offer PostAnyway, got {:?}",
        kinds,
    );
}

#[tokio::test]
async fn tier2_large_amount_does_not_trigger_when_clean() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let p = baseline_proposal_for(&seed);
    let result = validate_proposal(&pool, &p).await;
    assert!(!soft_codes(&result).contains(&SoftWarningCode::LargeAmount));
}

#[tokio::test]
async fn tier2_large_amount_edge_exact_threshold_passes() {
    // Boundary: `> 50_000` is strict. A line of exactly $500.00 must NOT
    // trigger. Use Credit Card again to avoid AbnormalBalance on Cash.
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let mut p = baseline_proposal_for(&seed);
    let cc_id = credit_card_account_id(&pool, &seed.household_id).await;
    p.lines[1].account_id = cc_id;
    p.lines[0].amount_cents = 50_000;
    p.lines[1].amount_cents = 50_000;

    let result = validate_proposal(&pool, &p).await;
    assert!(
        !soft_codes(&result).contains(&SoftWarningCode::LargeAmount),
        "amount exactly at threshold should pass LargeAmount, got {:?}",
        soft_codes(&result),
    );
}

// --- EnvelopeOverdraft ----------------------------------------------------
//
// Threshold: `spent + line.amount_cents > allocated` (strict). The seeded
// envelope starts at allocated=0, spent=0. Tests adjust `allocated` so the
// boundary is meaningful.
// Recovery: `PostAnyway` primary, `EditField` ("Change envelope") extra.
// No `Discard`.

#[tokio::test]
async fn tier2_envelope_overdraft_triggers() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    set_grocery_allocation(&pool, &seed.grocery_envelope_id, 1_000).await;
    let mut p = baseline_proposal_for(&seed);
    // Attach the envelope to the (debit) Groceries line. line amount=1500 >
    // allocated=1000 → overdraft.
    p.lines[0].envelope_id = Some(seed.grocery_envelope_id.clone());

    let result = validate_proposal(&pool, &p).await;

    assert!(
        soft_codes(&result).contains(&SoftWarningCode::EnvelopeOverdraft),
        "expected EnvelopeOverdraft but got: {:?}",
        soft_codes(&result),
    );
    let warn = find_soft(&result, SoftWarningCode::EnvelopeOverdraft)
        .expect("EnvelopeOverdraft warn present");
    let kinds = recovery_kinds_of_soft(warn);
    assert!(
        first_kind_is(&kinds, RecoveryKind::PostAnyway),
        "primary recovery for EnvelopeOverdraft: expected PostAnyway, got {:?}",
        kinds.first(),
    );
    assert!(
        kinds_contain(&kinds, RecoveryKind::EditField),
        "EnvelopeOverdraft should also offer EditField (Change envelope), got {:?}",
        kinds,
    );
}

#[tokio::test]
async fn tier2_envelope_overdraft_does_not_trigger_when_clean() {
    // No envelope attached to any line → rule cannot fire.
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let p = baseline_proposal_for(&seed);
    let result = validate_proposal(&pool, &p).await;
    assert!(!soft_codes(&result).contains(&SoftWarningCode::EnvelopeOverdraft));
}

#[tokio::test]
async fn tier2_envelope_overdraft_edge_exactly_at_cap_passes() {
    // Boundary: `spent + amount > allocated` is strict. With spent=0 and
    // allocated=1500, a line of exactly 1500 lands at the cap and must NOT
    // trigger.
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    set_grocery_allocation(&pool, &seed.grocery_envelope_id, 1_500).await;
    let mut p = baseline_proposal_for(&seed);
    p.lines[0].envelope_id = Some(seed.grocery_envelope_id.clone());
    // baseline amount is already 1500 — the exact boundary.

    let result = validate_proposal(&pool, &p).await;
    assert!(
        !soft_codes(&result).contains(&SoftWarningCode::EnvelopeOverdraft),
        "amount exactly at allocation should pass EnvelopeOverdraft, got {:?}",
        soft_codes(&result),
    );
}

// --- PossibleDuplicate ----------------------------------------------------
//
// Trigger: a posted transaction exists whose debit-side total equals the
// proposal's debit-side total, dated within `ONE_DAY_MS` of the proposal.
// Note: the rule does NOT filter by household, account, or payee — only by
// debit total + date proximity.
// Recovery: `PostAnyway` primary, `Discard` extra.

#[tokio::test]
async fn tier2_possible_duplicate_triggers() {
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let p = baseline_proposal_for(&seed);
    // Seed a posted txn one day before the proposal date with the same
    // $15.00 debit total.
    seed_posted_txn(
        &pool,
        &seed.household_id,
        &seed.expense_account_id,
        &seed.cash_account_id,
        1500,
        p.txn_date_ms - ONE_DAY_MS,
    )
    .await;

    let result = validate_proposal(&pool, &p).await;

    assert!(
        soft_codes(&result).contains(&SoftWarningCode::PossibleDuplicate),
        "expected PossibleDuplicate but got: {:?}",
        soft_codes(&result),
    );
    let warn = find_soft(&result, SoftWarningCode::PossibleDuplicate)
        .expect("PossibleDuplicate warn present");
    let kinds = recovery_kinds_of_soft(warn);
    assert!(
        first_kind_is(&kinds, RecoveryKind::PostAnyway),
        "primary recovery for PossibleDuplicate: expected PostAnyway, got {:?}",
        kinds.first(),
    );
    assert!(
        kinds_contain(&kinds, RecoveryKind::Discard),
        "PossibleDuplicate should also offer Discard, got {:?}",
        kinds,
    );
}

#[tokio::test]
async fn tier2_possible_duplicate_does_not_trigger_when_clean() {
    // No matching posted txn seeded → no duplicate.
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let p = baseline_proposal_for(&seed);
    let result = validate_proposal(&pool, &p).await;
    assert!(!soft_codes(&result).contains(&SoftWarningCode::PossibleDuplicate));
}

#[tokio::test]
async fn tier2_possible_duplicate_edge_outside_window_passes() {
    // Boundary: the rule looks for matches `WHERE ABS(t.txn_date - ?) <= ONE_DAY_MS`.
    // A posted txn dated more than one day from the proposal must NOT trigger,
    // even when the amount matches exactly.
    let pool = fresh_pool().await;
    let seed = seed_household(&pool).await;
    let p = baseline_proposal_for(&seed);
    seed_posted_txn(
        &pool,
        &seed.household_id,
        &seed.expense_account_id,
        &seed.cash_account_id,
        1500,
        p.txn_date_ms - 3 * ONE_DAY_MS,
    )
    .await;

    let result = validate_proposal(&pool, &p).await;
    assert!(
        !soft_codes(&result).contains(&SoftWarningCode::PossibleDuplicate),
        "match outside ±1-day window should not trigger PossibleDuplicate, got {:?}",
        soft_codes(&result),
    );
}

// === Task 6: Tier 3 (AIAdvisory) matrix ===================================
//
// Tier 3 advisories are produced by builder functions in `ai/advisories.rs`,
// NOT by the validator. Each builder is a pure function: given input, return
// an `AIAdvisory` with `user_message` and `NonEmpty<RecoveryAction>`. The
// matrix asserts (1) the user_message reflects the inputs and (2) the
// recovery action set matches what the builder emits.
//
// Recovery action expectations are verified against `advisories.rs` itself —
// *not* the design table — so the test reflects the actual builder output.
//
// Discoveries vs. the original task table for Tier 3:
//   - `unknown_payee`: extras are `PostAnyway` + `Discard` (not
//     `CreateMissing` + `Discard` as the table claimed). Primary `EditField`
//     matches.
//   - `suggested_account`: only one extra, `EditField`. No `Discard`. Primary
//     `UseSuggested` matches.
//   - `possible_duplicate`: matches the table — primary `PostAnyway`, extra
//     `Discard`.
//   - `envelope_near_limit`: extra is `ShowHelp` ("Review budget"), not
//     `EditField` + `Discard` as the table claimed. Primary `PostAnyway`
//     matches.

#[test]
fn tier3_unknown_payee_advisory_shape() {
    let advisory = advisories::unknown_payee("Trader Joe's");
    assert!(
        advisory.user_message.contains("Trader Joe's"),
        "user_message should reflect the payee name; got {:?}",
        advisory.user_message,
    );
    let kinds = recovery_kinds_of_advisory(&advisory);
    assert!(
        first_kind_is(&kinds, RecoveryKind::EditField),
        "primary kind: expected EditField, got {:?}",
        kinds.first(),
    );
    // advisories.rs emits PostAnyway + Discard as extras (the original task
    // table listed CreateMissing — the builder does not produce one).
    assert!(
        kinds_contain(&kinds, RecoveryKind::PostAnyway),
        "unknown_payee should also offer PostAnyway, got {:?}",
        kinds,
    );
    assert!(
        kinds_contain(&kinds, RecoveryKind::Discard),
        "unknown_payee should also offer Discard, got {:?}",
        kinds,
    );
}

#[test]
fn tier3_suggested_account_advisory_shape() {
    let advisory = advisories::suggested_account("Netflix", "Subscriptions");
    assert!(
        advisory.user_message.contains("Netflix"),
        "user_message should reflect the payee name; got {:?}",
        advisory.user_message,
    );
    assert!(
        advisory.user_message.contains("Subscriptions"),
        "user_message should reflect the account name; got {:?}",
        advisory.user_message,
    );
    let kinds = recovery_kinds_of_advisory(&advisory);
    assert!(
        first_kind_is(&kinds, RecoveryKind::UseSuggested),
        "primary kind: expected UseSuggested, got {:?}",
        kinds.first(),
    );
    // advisories.rs only emits EditField as the extra (no Discard, contrary
    // to the original task table).
    assert!(
        kinds_contain(&kinds, RecoveryKind::EditField),
        "suggested_account should also offer EditField, got {:?}",
        kinds,
    );
}

#[test]
fn tier3_possible_duplicate_advisory_shape() {
    let advisory = advisories::possible_duplicate(3);
    assert!(
        advisory.user_message.contains("3 days ago"),
        "user_message should reflect the days_ago argument; got {:?}",
        advisory.user_message,
    );
    let kinds = recovery_kinds_of_advisory(&advisory);
    assert!(
        first_kind_is(&kinds, RecoveryKind::PostAnyway),
        "primary kind: expected PostAnyway, got {:?}",
        kinds.first(),
    );
    assert!(
        kinds_contain(&kinds, RecoveryKind::Discard),
        "possible_duplicate should also offer Discard, got {:?}",
        kinds,
    );
}

#[test]
fn tier3_envelope_near_limit_advisory_shape() {
    let advisory = advisories::envelope_near_limit("Groceries", 95);
    assert!(
        advisory.user_message.contains("Groceries"),
        "user_message should reflect the envelope name; got {:?}",
        advisory.user_message,
    );
    assert!(
        advisory.user_message.contains("95%"),
        "user_message should reflect the percent_used; got {:?}",
        advisory.user_message,
    );
    let kinds = recovery_kinds_of_advisory(&advisory);
    assert!(
        first_kind_is(&kinds, RecoveryKind::PostAnyway),
        "primary kind: expected PostAnyway, got {:?}",
        kinds.first(),
    );
    // advisories.rs emits ShowHelp ("Review budget") as the only extra —
    // the original task table listed EditField + Discard, both wrong.
    assert!(
        kinds_contain(&kinds, RecoveryKind::ShowHelp),
        "envelope_near_limit should also offer ShowHelp, got {:?}",
        kinds,
    );
}
