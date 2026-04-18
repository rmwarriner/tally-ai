// Tier 3 AI advisories — T-025
// Four advisory constructors: unknown_payee, suggested_account,
// possible_duplicate, envelope_near_limit.
// These are informational only — they never block a commit.

use crate::core::validation::AIAdvisory;
use crate::error::{RecoveryAction, RecoveryKind};

pub fn unknown_payee(payee_name: &str) -> AIAdvisory {
    AIAdvisory {
        user_message: format!(
            "I didn't recognise \"{payee_name}\" — you may want to check the account I chose."
        ),
        actions: crate::error::NonEmpty::new(
            RecoveryAction { kind: RecoveryKind::EditField,   label: "Change account".to_string(), is_primary: true },
            vec![
                RecoveryAction { kind: RecoveryKind::PostAnyway, label: "Post anyway".to_string(),    is_primary: false },
                RecoveryAction { kind: RecoveryKind::Discard,    label: "Discard".to_string(),         is_primary: false },
            ],
        ),
    }
}

pub fn suggested_account(payee_name: &str, account_name: &str) -> AIAdvisory {
    AIAdvisory {
        user_message: format!(
            "Based on past entries I mapped \"{payee_name}\" to {account_name}. Let me know if that's wrong."
        ),
        actions: crate::error::NonEmpty::new(
            RecoveryAction { kind: RecoveryKind::UseSuggested, label: "Keep suggestion".to_string(), is_primary: true },
            vec![
                RecoveryAction { kind: RecoveryKind::EditField, label: "Change account".to_string(), is_primary: false },
            ],
        ),
    }
}

pub fn possible_duplicate(days_ago: u32) -> AIAdvisory {
    let when = if days_ago == 0 {
        "today".to_string()
    } else if days_ago == 1 {
        "yesterday".to_string()
    } else {
        format!("{days_ago} days ago")
    };
    AIAdvisory {
        user_message: format!(
            "This looks similar to a transaction from {when}. Double-check it isn't a duplicate."
        ),
        actions: crate::error::NonEmpty::new(
            RecoveryAction { kind: RecoveryKind::PostAnyway, label: "Post anyway".to_string(), is_primary: true },
            vec![
                RecoveryAction { kind: RecoveryKind::Discard, label: "Discard".to_string(), is_primary: false },
            ],
        ),
    }
}

pub fn envelope_near_limit(envelope_name: &str, percent_used: u8) -> AIAdvisory {
    AIAdvisory {
        user_message: format!(
            "The \"{envelope_name}\" budget is {percent_used}% used — you're getting close to the limit."
        ),
        actions: crate::error::NonEmpty::new(
            RecoveryAction { kind: RecoveryKind::PostAnyway, label: "Post anyway".to_string(), is_primary: true },
            vec![
                RecoveryAction { kind: RecoveryKind::ShowHelp, label: "Review budget".to_string(), is_primary: false },
            ],
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_payee_has_recovery_actions() {
        let a = unknown_payee("Costco");
        assert!(a.actions.len() >= 1);
        assert!(a.user_message.contains("Costco"));
    }

    #[test]
    fn unknown_payee_primary_action_is_edit() {
        let a = unknown_payee("Costco");
        assert!(matches!(a.actions.first().kind, RecoveryKind::EditField));
        assert!(a.actions.first().is_primary);
    }

    #[test]
    fn suggested_account_has_recovery_actions() {
        let a = suggested_account("Netflix", "Subscriptions");
        assert!(a.actions.len() >= 1);
        assert!(a.user_message.contains("Netflix"));
        assert!(a.user_message.contains("Subscriptions"));
    }

    #[test]
    fn suggested_account_primary_is_use_suggested() {
        let a = suggested_account("Netflix", "Subscriptions");
        assert!(matches!(a.actions.first().kind, RecoveryKind::UseSuggested));
        assert!(a.actions.first().is_primary);
    }

    #[test]
    fn possible_duplicate_today() {
        let a = possible_duplicate(0);
        assert!(a.user_message.contains("today"));
        assert!(a.actions.len() >= 1);
    }

    #[test]
    fn possible_duplicate_yesterday() {
        let a = possible_duplicate(1);
        assert!(a.user_message.contains("yesterday"));
    }

    #[test]
    fn possible_duplicate_days_ago() {
        let a = possible_duplicate(5);
        assert!(a.user_message.contains("5 days ago"));
    }

    #[test]
    fn possible_duplicate_primary_is_post_anyway() {
        let a = possible_duplicate(3);
        assert!(matches!(a.actions.first().kind, RecoveryKind::PostAnyway));
    }

    #[test]
    fn envelope_near_limit_has_recovery_actions() {
        let a = envelope_near_limit("Groceries", 85);
        assert!(a.actions.len() >= 1);
        assert!(a.user_message.contains("Groceries"));
        assert!(a.user_message.contains("85%"));
    }

    #[test]
    fn envelope_near_limit_primary_is_post_anyway() {
        let a = envelope_near_limit("Dining", 90);
        assert!(matches!(a.actions.first().kind, RecoveryKind::PostAnyway));
    }

    #[test]
    fn all_messages_contain_no_error_codes() {
        let advisories = vec![
            unknown_payee("X"),
            suggested_account("X", "Y"),
            possible_duplicate(2),
            envelope_near_limit("Z", 80),
        ];
        for a in &advisories {
            assert!(!a.user_message.contains("ERR_"));
            assert!(!a.user_message.contains("WARN_"));
            assert!(!a.user_message.contains("::"));
        }
    }
}
