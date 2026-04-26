//! T-060 — canonical inventory of validation behaviors.
//!
//! Every Tier 1 (HardError), Tier 2 (SoftWarning), and Tier 3 (AIAdvisory)
//! variant has at least one positive-trigger test, one non-trigger test, and
//! (where meaningful) one boundary/edge test. Each test asserts the expected
//! recovery action set against the spec, not just the error variant.

#![cfg(test)]

use sqlx::SqlitePool;

use crate::ai::advisories;
use crate::core::proposal::{ProposedLine, Side, TransactionProposal};
use crate::core::validation::{
    validate_proposal, AIAdvisory, HardError, HardErrorCode, SoftWarning, SoftWarningCode,
    ValidationResult,
};
use crate::error::{RecoveryAction, RecoveryKind};

// Shared fixture helpers ---------------------------------------------------

async fn fresh_pool() -> SqlitePool {
    // Use whatever in-memory pool helper validation.rs::tests uses.
    // See Task 3 for adapting this to the actual helper name.
    todo!("see Task 3")
}

fn baseline_proposal_for(_seed: &SeedIds) -> TransactionProposal {
    todo!("see Task 3 — fill in once seed accounts exist")
}

struct SeedIds {
    household_id: String,
    cash_account_id: String,
    expense_account_id: String,
    grocery_envelope_id: Option<String>,
}

async fn seed_household(_pool: &SqlitePool) -> SeedIds {
    todo!("see Task 3")
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
