use serde::{Deserialize, Serialize};

use crate::error::{NonEmpty, RecoveryAction};

// Three-tier validation — blocking hard errors, non-blocking soft warnings, AI-only advisories.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HardErrorCode {
    NoLines,
    UnbalancedLines,
    ZeroAmount,
    NegativeAmount,
    UnknownAccount,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::RecoveryKind;

    fn discard_action() -> RecoveryAction {
        RecoveryAction {
            kind: RecoveryKind::Discard,
            label: "Discard".to_string(),
            is_primary: false,
        }
    }

    fn edit_action() -> RecoveryAction {
        RecoveryAction {
            kind: RecoveryKind::EditField,
            label: "Edit".to_string(),
            is_primary: true,
        }
    }

    fn post_anyway_action() -> RecoveryAction {
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
            edit_action(),
            vec![discard_action()],
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
            post_anyway_action(),
            vec![discard_action()],
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
            edit_action(),
            vec![],
        );
        assert!(!error.actions.as_slice().is_empty());
    }

    #[test]
    fn soft_warning_always_has_recovery_actions() {
        let warning = soft_warning(
            SoftWarningCode::PossibleDuplicate,
            "This looks like a transaction you've already recorded.",
            post_anyway_action(),
            vec![discard_action()],
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
            edit_action(),
            vec![],
        );
        let e2 = hard_error(
            HardErrorCode::ZeroAmount,
            "An entry has a zero amount.",
            edit_action(),
            vec![discard_action()],
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
            post_anyway_action(),
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
}
