use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::core::proposal::{Side, TransactionProposal};
use crate::error::{NonEmpty, RecoveryAction, RecoveryKind};

// Three-tier validation — blocking hard errors, non-blocking soft warnings, AI-only advisories.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HardErrorCode {
    NoLines,
    UnbalancedLines,
    ZeroAmount,
    NegativeAmount,
    UnknownAccount,
    PlaceholderAccount,
    AbnormalBalance,
    EnvelopeMismatch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardError {
    pub code: HardErrorCode,
    /// Plain-language message — no error codes or field names exposed to the user.
    pub user_message: String,
    pub actions: NonEmpty<RecoveryAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SoftWarningCode {
    EnvelopeBudgetExceeded,
    PossibleDuplicate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftWarning {
    pub code: SoftWarningCode,
    pub user_message: String,
    pub actions: NonEmpty<RecoveryAction>,
}

/// Informational advisory produced only by the AI layer; never blocks commit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIAdvisory {
    pub user_message: String,
    pub actions: NonEmpty<RecoveryAction>,
}

/// The Rust core's answer after validating a TransactionProposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ValidationResult {
    /// Proposal is clean — safe to commit.
    Accepted,
    /// Proposal has non-blocking warnings; user must acknowledge before commit.
    Warnings {
        warnings: NonEmpty<SoftWarning>,
        advisories: Vec<AIAdvisory>,
    },
    /// Proposal has blocking errors — must be corrected before commit.
    Rejected {
        errors: NonEmpty<HardError>,
        warnings: Vec<SoftWarning>,
    },
}

impl ValidationResult {
    pub fn is_accepted(&self) -> bool {
        matches!(self, ValidationResult::Accepted)
    }

    pub fn is_rejected(&self) -> bool {
        matches!(self, ValidationResult::Rejected { .. })
    }
}

// -- helpers for building common results --

pub fn hard_error(
    code: HardErrorCode,
    user_message: impl Into<String>,
    primary_action: RecoveryAction,
    extra_actions: Vec<RecoveryAction>,
) -> HardError {
    HardError {
        code,
        user_message: user_message.into(),
        actions: NonEmpty::new(primary_action, extra_actions),
    }
}

pub fn soft_warning(
    code: SoftWarningCode,
    user_message: impl Into<String>,
    primary_action: RecoveryAction,
    extra_actions: Vec<RecoveryAction>,
) -> SoftWarning {
    SoftWarning {
        code,
        user_message: user_message.into(),
        actions: NonEmpty::new(primary_action, extra_actions),
    }
}

pub fn ai_advisory(
    user_message: impl Into<String>,
    primary_action: RecoveryAction,
    extra_actions: Vec<RecoveryAction>,
) -> AIAdvisory {
    AIAdvisory {
        user_message: user_message.into(),
        actions: NonEmpty::new(primary_action, extra_actions),
    }
}

fn edit_action(label: &str) -> RecoveryAction {
    RecoveryAction {
        kind: RecoveryKind::EditField,
        label: label.to_string(),
        is_primary: true,
    }
}

fn discard_action() -> RecoveryAction {
    RecoveryAction {
        kind: RecoveryKind::Discard,
        label: "Discard".to_string(),
        is_primary: false,
    }
}

// -- validation engine --

#[derive(sqlx::FromRow)]
struct AccountRow {
    id: String,
    normal_balance: String,
    is_placeholder: bool,
}

#[derive(sqlx::FromRow)]
struct BalanceRow {
    debit_total: i64,
    credit_total: i64,
}

/// Validates a TransactionProposal against all Tier 1 hard rules.
/// DB errors during account checks are treated as "account not found" rather than panics.
pub async fn validate_proposal(
    pool: &SqlitePool,
    proposal: &TransactionProposal,
) -> ValidationResult {
    let mut errors: Vec<HardError> = Vec::new();

    // ERR_INSUFFICIENT_LINES
    if proposal.lines.len() < 2 {
        errors.push(hard_error(
            HardErrorCode::NoLines,
            "A transaction must have at least two entries.",
            edit_action("Add entries"),
            vec![discard_action()],
        ));
    }

    // ERR_INVALID_AMOUNT
    if proposal.lines.iter().any(|l| l.amount_cents == 0) {
        errors.push(hard_error(
            HardErrorCode::ZeroAmount,
            "All amounts must be greater than zero.",
            edit_action("Fix amount"),
            vec![discard_action()],
        ));
    }
    if proposal.lines.iter().any(|l| l.amount_cents < 0) {
        errors.push(hard_error(
            HardErrorCode::NegativeAmount,
            "Amounts cannot be negative. Use debit or credit to indicate direction.",
            edit_action("Fix amount"),
            vec![discard_action()],
        ));
    }

    // ERR_UNBALANCED
    let debit_sum: i64 = proposal
        .lines
        .iter()
        .filter(|l| matches!(l.side, Side::Debit))
        .map(|l| l.amount_cents)
        .sum();
    let credit_sum: i64 = proposal
        .lines
        .iter()
        .filter(|l| matches!(l.side, Side::Credit))
        .map(|l| l.amount_cents)
        .sum();
    if debit_sum != credit_sum {
        errors.push(hard_error(
            HardErrorCode::UnbalancedLines,
            "The debit and credit totals must be equal.",
            edit_action("Fix amounts"),
            vec![discard_action()],
        ));
    }

    // DB checks: ERR_INVALID_ACCOUNT, ERR_PLACEHOLDER_ACCOUNT, ERR_ABNORMAL_BALANCE
    validate_accounts(pool, proposal, &mut errors).await;

    if errors.is_empty() {
        ValidationResult::Accepted
    } else {
        let mut iter = errors.into_iter();
        let first = iter.next().unwrap(); // safe: non-empty
        ValidationResult::Rejected {
            errors: NonEmpty::new(first, iter.collect()),
            warnings: vec![],
        }
    }
}

async fn validate_accounts(
    pool: &SqlitePool,
    proposal: &TransactionProposal,
    errors: &mut Vec<HardError>,
) {
    let mut seen = std::collections::HashSet::new();
    let unique_ids: Vec<&str> = proposal
        .lines
        .iter()
        .map(|l| l.account_id.as_str())
        .filter(|id| seen.insert(*id))
        .collect();

    for account_id in unique_ids {
        let row: Option<AccountRow> = sqlx::query_as(
            "SELECT id, normal_balance, is_placeholder FROM accounts WHERE id = ?",
        )
        .bind(account_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

        match row {
            None => {
                errors.push(hard_error(
                    HardErrorCode::UnknownAccount,
                    "One or more accounts could not be found.",
                    RecoveryAction {
                        kind: RecoveryKind::CreateMissing,
                        label: "Create account".to_string(),
                        is_primary: true,
                    },
                    vec![discard_action()],
                ));
            }
            Some(account) => {
                if account.is_placeholder {
                    errors.push(hard_error(
                        HardErrorCode::PlaceholderAccount,
                        "You cannot post to a grouping account. Choose a specific account instead.",
                        edit_action("Change account"),
                        vec![discard_action()],
                    ));
                }

                check_abnormal_balance(pool, proposal, &account, errors).await;
            }
        }
    }
}

async fn check_abnormal_balance(
    pool: &SqlitePool,
    proposal: &TransactionProposal,
    account: &AccountRow,
    errors: &mut Vec<HardError>,
) {
    let balance: BalanceRow = sqlx::query_as(
        "SELECT
            COALESCE(SUM(CASE WHEN jl.side = 'debit'  THEN jl.amount ELSE 0 END), 0) AS debit_total,
            COALESCE(SUM(CASE WHEN jl.side = 'credit' THEN jl.amount ELSE 0 END), 0) AS credit_total
         FROM journal_lines jl
         JOIN transactions t ON t.id = jl.transaction_id
         WHERE jl.account_id = ? AND t.status = 'posted'",
    )
    .bind(&account.id)
    .fetch_one(pool)
    .await
    .unwrap_or(BalanceRow {
        debit_total: 0,
        credit_total: 0,
    });

    let current = if account.normal_balance == "debit" {
        balance.debit_total - balance.credit_total
    } else {
        balance.credit_total - balance.debit_total
    };

    let proposal_debit: i64 = proposal
        .lines
        .iter()
        .filter(|l| l.account_id == account.id && matches!(l.side, Side::Debit))
        .map(|l| l.amount_cents)
        .sum();
    let proposal_credit: i64 = proposal
        .lines
        .iter()
        .filter(|l| l.account_id == account.id && matches!(l.side, Side::Credit))
        .map(|l| l.amount_cents)
        .sum();

    let proposal_net = if account.normal_balance == "debit" {
        proposal_debit - proposal_credit
    } else {
        proposal_credit - proposal_debit
    };

    if current + proposal_net < 0 {
        errors.push(hard_error(
            HardErrorCode::AbnormalBalance,
            "This transaction would leave an account with an abnormal balance.",
            edit_action("Fix entries"),
            vec![
                RecoveryAction {
                    kind: RecoveryKind::PostAnyway,
                    label: "Post anyway".to_string(),
                    is_primary: false,
                },
                discard_action(),
            ],
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::proposal::ProposedLine;
    use crate::db::{connection::create_encrypted_db, migrations::run_migrations};
    use tempfile::tempdir;

    // -- type-shape tests (unchanged from before) --

    fn discard() -> RecoveryAction {
        RecoveryAction {
            kind: RecoveryKind::Discard,
            label: "Discard".to_string(),
            is_primary: false,
        }
    }

    fn edit() -> RecoveryAction {
        RecoveryAction {
            kind: RecoveryKind::EditField,
            label: "Edit".to_string(),
            is_primary: true,
        }
    }

    fn post_anyway() -> RecoveryAction {
        RecoveryAction {
            kind: RecoveryKind::PostAnyway,
            label: "Post anyway".to_string(),
            is_primary: true,
        }
    }

    #[test]
    fn validation_result_accepted_roundtrips_json() {
        let result = ValidationResult::Accepted;
        let json = serde_json::to_string(&result).expect("serialize");
        let back: ValidationResult = serde_json::from_str(&json).expect("deserialize");
        assert!(back.is_accepted());
    }

    #[test]
    fn validation_result_rejected_carries_hard_errors() {
        let error = hard_error(
            HardErrorCode::UnbalancedLines,
            "The amounts don't balance. Please check your entries.",
            edit(),
            vec![discard()],
        );

        let result = ValidationResult::Rejected {
            errors: NonEmpty::new(error, vec![]),
            warnings: vec![],
        };

        assert!(result.is_rejected());
        assert!(!result.is_accepted());

        let json = serde_json::to_string(&result).expect("serialize");
        let back: ValidationResult = serde_json::from_str(&json).expect("deserialize");
        assert!(back.is_rejected());
    }

    #[test]
    fn validation_result_warnings_roundtrips_json() {
        let warning = soft_warning(
            SoftWarningCode::EnvelopeBudgetExceeded,
            "This would put your grocery budget over the limit.",
            post_anyway(),
            vec![discard()],
        );

        let result = ValidationResult::Warnings {
            warnings: NonEmpty::new(warning, vec![]),
            advisories: vec![],
        };

        assert!(!result.is_accepted());
        assert!(!result.is_rejected());

        let json = serde_json::to_string(&result).expect("serialize");
        let back: ValidationResult = serde_json::from_str(&json).expect("deserialize");
        assert!(!back.is_accepted());
    }

    #[test]
    fn hard_error_always_has_recovery_actions() {
        let error = hard_error(
            HardErrorCode::NoLines,
            "A transaction needs at least two entries.",
            edit(),
            vec![],
        );
        assert!(!error.actions.as_slice().is_empty());
    }

    #[test]
    fn soft_warning_always_has_recovery_actions() {
        let warning = soft_warning(
            SoftWarningCode::PossibleDuplicate,
            "This looks like a transaction you've already recorded.",
            post_anyway(),
            vec![discard()],
        );
        assert!(!warning.actions.as_slice().is_empty());
        assert_eq!(warning.actions.as_slice().len(), 2);
    }

    #[test]
    fn ai_advisory_always_has_recovery_actions() {
        let advisory = ai_advisory(
            "This is larger than your typical grocery trip.",
            RecoveryAction {
                kind: RecoveryKind::ShowHelp,
                label: "Learn more".to_string(),
                is_primary: false,
            },
            vec![],
        );
        assert!(!advisory.actions.as_slice().is_empty());
    }

    #[test]
    fn hard_error_codes_serialize_screaming_snake_case() {
        let json = serde_json::to_string(&HardErrorCode::UnknownAccount).expect("serialize");
        assert_eq!(json, "\"UNKNOWN_ACCOUNT\"");

        let json = serde_json::to_string(&HardErrorCode::EnvelopeMismatch).expect("serialize");
        assert_eq!(json, "\"ENVELOPE_MISMATCH\"");

        let json = serde_json::to_string(&HardErrorCode::PlaceholderAccount).expect("serialize");
        assert_eq!(json, "\"PLACEHOLDER_ACCOUNT\"");

        let json = serde_json::to_string(&HardErrorCode::AbnormalBalance).expect("serialize");
        assert_eq!(json, "\"ABNORMAL_BALANCE\"");
    }

    #[test]
    fn soft_warning_codes_serialize_screaming_snake_case() {
        let json =
            serde_json::to_string(&SoftWarningCode::EnvelopeBudgetExceeded).expect("serialize");
        assert_eq!(json, "\"ENVELOPE_BUDGET_EXCEEDED\"");
    }

    #[test]
    fn rejected_result_can_carry_multiple_errors() {
        let e1 = hard_error(
            HardErrorCode::NoLines,
            "A transaction needs at least two entries.",
            edit(),
            vec![],
        );
        let e2 = hard_error(
            HardErrorCode::ZeroAmount,
            "An entry has a zero amount.",
            edit(),
            vec![discard()],
        );

        let result = ValidationResult::Rejected {
            errors: NonEmpty::new(e1, vec![e2]),
            warnings: vec![],
        };

        if let ValidationResult::Rejected { errors, .. } = &result {
            assert_eq!(errors.as_slice().len(), 2);
        } else {
            panic!("expected Rejected");
        }
    }

    #[test]
    fn warnings_result_can_carry_advisories() {
        let warning = soft_warning(
            SoftWarningCode::EnvelopeBudgetExceeded,
            "Over budget.",
            post_anyway(),
            vec![],
        );
        let advisory = ai_advisory(
            "This is an unusual amount for this category.",
            RecoveryAction {
                kind: RecoveryKind::ShowHelp,
                label: "Learn more".to_string(),
                is_primary: false,
            },
            vec![],
        );

        let result = ValidationResult::Warnings {
            warnings: NonEmpty::new(warning, vec![]),
            advisories: vec![advisory],
        };

        if let ValidationResult::Warnings { advisories, .. } = &result {
            assert_eq!(advisories.len(), 1);
        } else {
            panic!("expected Warnings");
        }
    }

    // -- validate_proposal tests --

    async fn test_pool() -> SqlitePool {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.keep().join("test.db");
        let pool = create_encrypted_db(&db_path, "test", &[0u8; 16])
            .await
            .expect("create db");
        run_migrations(&pool).await.expect("migrate");
        pool
    }

    fn debit_line(account_id: &str, cents: i64) -> ProposedLine {
        ProposedLine {
            account_id: account_id.to_string(),
            envelope_id: None,
            amount_cents: cents,
            side: Side::Debit,
        }
    }

    fn credit_line(account_id: &str, cents: i64) -> ProposedLine {
        ProposedLine {
            account_id: account_id.to_string(),
            envelope_id: None,
            amount_cents: cents,
            side: Side::Credit,
        }
    }

    fn proposal(lines: Vec<ProposedLine>) -> TransactionProposal {
        TransactionProposal {
            memo: None,
            txn_date_ms: 1_700_000_000_000,
            lines,
        }
    }

    async fn insert_account(
        pool: &SqlitePool,
        household_id: &str,
        id: &str,
        normal_balance: &str,
        is_placeholder: bool,
    ) {
        let acct_type = if normal_balance == "debit" {
            "asset"
        } else {
            "income"
        };
        sqlx::query(
            "INSERT INTO accounts (id, household_id, name, type, normal_balance, is_placeholder, created_at)
             VALUES (?, ?, ?, ?, ?, ?, 0)",
        )
        .bind(id)
        .bind(household_id)
        .bind(id)
        .bind(acct_type)
        .bind(normal_balance)
        .bind(is_placeholder as i64)
        .execute(pool)
        .await
        .expect("insert account");
    }

    async fn insert_household(pool: &SqlitePool, id: &str) {
        sqlx::query(
            "INSERT INTO households (id, name, timezone, created_at) VALUES (?, 'Test', 'UTC', 0)",
        )
        .bind(id)
        .execute(pool)
        .await
        .expect("insert household");
    }

    fn error_codes(result: &ValidationResult) -> Vec<HardErrorCode> {
        match result {
            ValidationResult::Rejected { errors, .. } => {
                errors.as_slice().iter().map(|e| e.code).collect()
            }
            _ => vec![],
        }
    }

    #[tokio::test]
    async fn validate_rejects_single_line() {
        let pool = test_pool().await;
        let p = proposal(vec![debit_line("acc", 100)]);
        let result = validate_proposal(&pool, &p).await;
        assert!(result.is_rejected());
        assert!(error_codes(&result).contains(&HardErrorCode::NoLines));
    }

    #[tokio::test]
    async fn validate_rejects_zero_amount() {
        let pool = test_pool().await;
        let p = proposal(vec![debit_line("acc_a", 0), credit_line("acc_b", 0)]);
        let result = validate_proposal(&pool, &p).await;
        assert!(result.is_rejected());
        assert!(error_codes(&result).contains(&HardErrorCode::ZeroAmount));
    }

    #[tokio::test]
    async fn validate_rejects_negative_amount() {
        let pool = test_pool().await;
        let p = proposal(vec![debit_line("acc_a", -50), credit_line("acc_b", -50)]);
        let result = validate_proposal(&pool, &p).await;
        assert!(result.is_rejected());
        assert!(error_codes(&result).contains(&HardErrorCode::NegativeAmount));
    }

    #[tokio::test]
    async fn validate_rejects_unbalanced_lines() {
        let pool = test_pool().await;
        let p = proposal(vec![debit_line("acc_a", 100), credit_line("acc_b", 200)]);
        let result = validate_proposal(&pool, &p).await;
        assert!(result.is_rejected());
        assert!(error_codes(&result).contains(&HardErrorCode::UnbalancedLines));
    }

    #[tokio::test]
    async fn validate_rejects_unknown_account() {
        let pool = test_pool().await;
        let p = proposal(vec![
            debit_line("no_such_account", 100),
            credit_line("also_missing", 100),
        ]);
        let result = validate_proposal(&pool, &p).await;
        assert!(result.is_rejected());
        assert!(error_codes(&result).contains(&HardErrorCode::UnknownAccount));
    }

    #[tokio::test]
    async fn validate_rejects_placeholder_account() {
        let pool = test_pool().await;
        insert_household(&pool, "hh1").await;
        insert_account(&pool, "hh1", "acc_placeholder", "debit", true).await;
        insert_account(&pool, "hh1", "acc_real", "credit", false).await;

        let p = proposal(vec![
            debit_line("acc_placeholder", 100),
            credit_line("acc_real", 100),
        ]);
        let result = validate_proposal(&pool, &p).await;
        assert!(result.is_rejected());
        assert!(error_codes(&result).contains(&HardErrorCode::PlaceholderAccount));
    }

    #[tokio::test]
    async fn validate_rejects_abnormal_balance() {
        let pool = test_pool().await;
        insert_household(&pool, "hh2").await;
        // checking: asset (debit-normal). Pre-seed $100 balance via a prior posted txn.
        insert_account(&pool, "hh2", "acc_checking", "debit", false).await;
        insert_account(&pool, "hh2", "acc_income", "credit", false).await;

        // Seed $100 debit balance on checking
        sqlx::query(
            "INSERT INTO transactions (id, household_id, txn_date, entry_date, status, source, created_at)
             VALUES ('seed_txn', 'hh2', 0, 0, 'posted', 'manual', 0)",
        )
        .execute(&pool)
        .await
        .expect("seed txn");
        sqlx::query(
            "INSERT INTO journal_lines (id, transaction_id, account_id, amount, side, created_at)
             VALUES ('seed_jl', 'seed_txn', 'acc_checking', 10000, 'debit', 0)",
        )
        .execute(&pool)
        .await
        .expect("seed line");

        // Now propose crediting checking by $200 → balance would be 100 - 200 = -100 (abnormal)
        let p = proposal(vec![
            credit_line("acc_checking", 20000),
            debit_line("acc_income", 20000),
        ]);
        let result = validate_proposal(&pool, &p).await;
        assert!(result.is_rejected());
        assert!(error_codes(&result).contains(&HardErrorCode::AbnormalBalance));
    }

    #[tokio::test]
    async fn validate_accepts_valid_proposal() {
        let pool = test_pool().await;
        insert_household(&pool, "hh3").await;
        // checking (debit-normal) and income (credit-normal): receiving $100 income
        insert_account(&pool, "hh3", "acc_checking", "debit", false).await;
        insert_account(&pool, "hh3", "acc_income", "credit", false).await;

        // Debit checking $100 (balance 0→+100, normal for debit account)
        // Credit income $100 (balance 0→+100, normal for credit account)
        let p = proposal(vec![
            debit_line("acc_checking", 10000),
            credit_line("acc_income", 10000),
        ]);
        let result = validate_proposal(&pool, &p).await;
        assert!(result.is_accepted(), "expected Accepted, got {result:?}");
    }

    #[tokio::test]
    async fn validate_collects_multiple_errors() {
        let pool = test_pool().await;
        // Single line (NoLines) + unbalanced (amounts differ) + unknown accounts
        let p = proposal(vec![debit_line("ghost_a", 100), credit_line("ghost_b", 200)]);
        let result = validate_proposal(&pool, &p).await;
        assert!(result.is_rejected());
        let codes = error_codes(&result);
        assert!(codes.contains(&HardErrorCode::UnbalancedLines));
        assert!(codes.contains(&HardErrorCode::UnknownAccount));
        // At least 2 distinct errors
        assert!(codes.len() >= 2);
    }
}
