// Envelope triggers — T-051.
//
// Two flavors fire post-commit:
//   - EnvelopeOver         spent_cents > allocated_cents
//   - EnvelopeApproaching  spent_cents >= 85% allocated AND not over
//
// Envelopes with allocated == 0 (no budget set yet) are skipped so the user
// isn't pestered before they've configured a target.

use sqlx::SqlitePool;

use crate::core::insight::{InsightDraft, InsightKind};
use crate::core::read::{current_envelope_periods, EnvelopeStatus};

const APPROACHING_RATIO_NUMERATOR: i64 = 85;
const APPROACHING_RATIO_DENOMINATOR: i64 = 100;

/// Returns drafts for every envelope crossing a threshold at `as_of_ms`.
/// Producers gate by sensitivity + dedup via `core::insight::log_if_new`.
pub async fn evaluate(
    pool: &SqlitePool,
    household_id: &str,
    as_of_ms: i64,
) -> Result<Vec<InsightDraft>, sqlx::Error> {
    let periods = current_envelope_periods(pool, household_id, as_of_ms).await?;
    Ok(periods.into_iter().filter_map(draft_for).collect())
}

fn draft_for(status: EnvelopeStatus) -> Option<InsightDraft> {
    if status.allocated_cents <= 0 {
        return None;
    }

    if status.spent_cents > status.allocated_cents {
        let over_by = status.spent_cents - status.allocated_cents;
        return Some(InsightDraft {
            kind: InsightKind::EnvelopeOver,
            entity_id: Some(status.envelope_id),
            user_message: format!(
                "{} is over budget by {}.",
                status.name,
                format_cents(over_by),
            ),
            extra: Some(serde_json::json!({
                "envelope_name": status.name,
                "allocated_cents": status.allocated_cents,
                "spent_cents": status.spent_cents,
                "over_by_cents": over_by,
            })),
        });
    }

    // Approaching: spent / allocated >= 85% (use integer math, no f64).
    let threshold_num = status.allocated_cents * APPROACHING_RATIO_NUMERATOR;
    let scaled_spent = status.spent_cents * APPROACHING_RATIO_DENOMINATOR;
    if scaled_spent >= threshold_num {
        let remaining = status.allocated_cents - status.spent_cents;
        return Some(InsightDraft {
            kind: InsightKind::EnvelopeApproaching,
            entity_id: Some(status.envelope_id),
            user_message: format!(
                "{} is close to its limit — {} left.",
                status.name,
                format_cents(remaining),
            ),
            extra: Some(serde_json::json!({
                "envelope_name": status.name,
                "allocated_cents": status.allocated_cents,
                "spent_cents": status.spent_cents,
                "remaining_cents": remaining,
            })),
        });
    }

    None
}

fn format_cents(cents: i64) -> String {
    let dollars = cents / 100;
    let remainder = (cents % 100).abs();
    format!("${dollars}.{remainder:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(name: &str, allocated: i64, spent: i64) -> EnvelopeStatus {
        EnvelopeStatus {
            envelope_id: format!("env_{name}"),
            name: name.to_string(),
            allocated_cents: allocated,
            spent_cents: spent,
        }
    }

    #[test]
    fn over_emits_envelope_over_with_amount() {
        let draft = draft_for(status("Groceries", 50_000, 60_000)).unwrap();
        assert_eq!(draft.kind, InsightKind::EnvelopeOver);
        assert!(draft.user_message.contains("Groceries"));
        assert!(draft.user_message.contains("$100.00"));
    }

    #[test]
    fn at_or_above_85_percent_emits_approaching() {
        let draft = draft_for(status("Gas", 10_000, 8_500)).unwrap();
        assert_eq!(draft.kind, InsightKind::EnvelopeApproaching);
        assert!(draft.user_message.contains("$15.00"));
    }

    #[test]
    fn below_85_percent_emits_nothing() {
        assert!(draft_for(status("Gas", 10_000, 8_499)).is_none());
        assert!(draft_for(status("Gas", 10_000, 0)).is_none());
    }

    #[test]
    fn zero_allocated_emits_nothing_even_if_over() {
        // Envelope with no budget set yet — don't pester the user.
        assert!(draft_for(status("Misc", 0, 5_000)).is_none());
    }

    #[test]
    fn exactly_at_budget_is_approaching_not_over() {
        let draft = draft_for(status("Books", 1_000, 1_000)).unwrap();
        assert_eq!(draft.kind, InsightKind::EnvelopeApproaching);
    }

    #[test]
    fn one_cent_over_is_envelope_over() {
        let draft = draft_for(status("Books", 1_000, 1_001)).unwrap();
        assert_eq!(draft.kind, InsightKind::EnvelopeOver);
    }

    #[test]
    fn entity_id_is_envelope_id() {
        let draft = draft_for(status("Groceries", 100, 200)).unwrap();
        assert_eq!(draft.entity_id.as_deref(), Some("env_Groceries"));
    }
}
