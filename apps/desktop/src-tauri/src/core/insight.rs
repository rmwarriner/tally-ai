// Insight log + emit gating — T-053.
//
// Backs the proactive engine: every produced insight (envelope alerts,
// duplicate detection, morning briefing) goes through `log_if_new`, which
// uses the unique key (household, kind, entity, day_bucket) to guarantee
// at-most-once-per-day-per-entity emission.
//
// The shape returned to the frontend is intentionally flat — it round-trips
// to the existing `proactive` chat message kind so we don't need a new
// message variant in the UI.

use chrono::{DateTime, TimeZone};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use thiserror::Error;

use crate::id::new_ulid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsightKind {
    /// An envelope's spent_cents has crossed its allocated_cents.
    EnvelopeOver,
    /// Spent has crossed the 85% threshold but is still under budget.
    EnvelopeApproaching,
    /// A proposal looks like a duplicate of a recent posted transaction.
    PossibleDuplicate,
    /// First session of the household-local day — assembled briefing.
    MorningBriefing,
}

impl InsightKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EnvelopeOver => "envelope_over",
            Self::EnvelopeApproaching => "envelope_approaching",
            Self::PossibleDuplicate => "possible_duplicate",
            Self::MorningBriefing => "morning_briefing",
        }
    }

    /// Spec §6.2 categories — drives the left-border accent on the proactive
    /// bubble. Alerts are red, insights amber, briefings blue.
    pub fn category(&self) -> InsightCategory {
        match self {
            Self::EnvelopeOver => InsightCategory::Alert,
            Self::EnvelopeApproaching => InsightCategory::Insight,
            Self::PossibleDuplicate => InsightCategory::Insight,
            Self::MorningBriefing => InsightCategory::Briefing,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsightCategory {
    Alert,
    Insight,
    Briefing,
}

/// Sensitivity gate (T-054). Each insight kind declares the *minimum*
/// level at which it should surface; a household setting at or above that
/// floor lets the insight through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sensitivity {
    Quiet,
    Normal,
    Proactive,
}

impl Sensitivity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Quiet => "quiet",
            Self::Normal => "normal",
            Self::Proactive => "proactive",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "quiet" => Some(Self::Quiet),
            "normal" => Some(Self::Normal),
            "proactive" => Some(Self::Proactive),
            _ => None,
        }
    }
}

impl InsightKind {
    /// Minimum sensitivity at which this kind surfaces to the user. Quiet
    /// always suppresses; Normal lets through hard alerts; Proactive lets
    /// everything through.
    fn min_sensitivity(&self) -> Sensitivity {
        match self {
            Self::EnvelopeOver | Self::PossibleDuplicate => Sensitivity::Normal,
            Self::EnvelopeApproaching | Self::MorningBriefing => Sensitivity::Proactive,
        }
    }
}

pub fn should_emit(kind: InsightKind, sensitivity: Sensitivity) -> bool {
    sensitivity >= kind.min_sensitivity()
}

/// What a producer hands to `log_if_new`. The `entity_id` discriminates
/// per-row dedup; pass `None` for kind-level singletons (the briefing).
#[derive(Debug, Clone)]
pub struct InsightDraft {
    pub kind: InsightKind,
    pub entity_id: Option<String>,
    pub user_message: String,
    /// Optional kind-specific JSON merged into the persisted payload.
    pub extra: Option<serde_json::Value>,
}

/// Wire shape returned to the frontend. Mirrors the `proactive` chat
/// message variant so the UI can render directly without a translator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProactiveInsight {
    pub id: String,
    pub kind: InsightKind,
    pub category: InsightCategory,
    pub user_message: String,
    pub created_at: i64,
}

#[derive(Debug, Error)]
pub enum InsightError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Invalid timezone: {0}")]
    Timezone(String),
    #[error("JSON serialization failed: {0}")]
    Json(#[from] serde_json::Error),
}

/// Returns the unix-ms of household-local midnight for `now_ms`. Used as
/// the dedup `day_bucket`.
pub fn day_bucket_ms(tz: &str, now_ms: i64) -> Result<i64, InsightError> {
    let tz: Tz = tz.parse().map_err(|e: chrono_tz::ParseError| {
        InsightError::Timezone(format!("'{tz}': {e}"))
    })?;
    let now = DateTime::from_timestamp_millis(now_ms)
        .ok_or_else(|| InsightError::Timezone(format!("invalid unix-ms: {now_ms}")))?;
    let local = now.with_timezone(&tz);
    let date = local.date_naive();
    let midnight_local = tz
        .from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
        .single()
        .ok_or_else(|| InsightError::Timezone("ambiguous local midnight".into()))?;
    Ok(midnight_local.timestamp_millis())
}

/// Inserts the draft if no row yet exists for (household, kind, entity,
/// day_bucket). Returns the wire-shape on insert; `None` when dedup'd.
///
/// Caller is responsible for sensitivity gating via `should_emit`.
pub async fn log_if_new(
    pool: &SqlitePool,
    household_id: &str,
    tz: &str,
    now_ms: i64,
    draft: InsightDraft,
) -> Result<Option<ProactiveInsight>, InsightError> {
    let day_bucket = day_bucket_ms(tz, now_ms)?;
    let id = new_ulid();
    let payload_json = serde_json::to_string(&PersistedPayload {
        user_message: &draft.user_message,
        extra: draft.extra.as_ref(),
    })?;

    let result = sqlx::query(
        "INSERT INTO insight_log (id, household_id, kind, entity_id, day_bucket, payload, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(household_id)
    .bind(draft.kind.as_str())
    .bind(&draft.entity_id)
    .bind(day_bucket)
    .bind(&payload_json)
    .bind(now_ms)
    .execute(pool)
    .await;

    match result {
        Ok(_) => Ok(Some(ProactiveInsight {
            id,
            kind: draft.kind,
            category: draft.kind.category(),
            user_message: draft.user_message,
            created_at: now_ms,
        })),
        Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => Ok(None),
        Err(other) => Err(other.into()),
    }
}

#[derive(Serialize)]
struct PersistedPayload<'a> {
    user_message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    extra: Option<&'a serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::create_encrypted_db;
    use tempfile::tempdir;

    async fn test_pool() -> (SqlitePool, String) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("insight.db");
        let pool = create_encrypted_db(&path, "pp", &[0u8; 16]).await.unwrap();
        let hh = new_ulid();
        sqlx::query("INSERT INTO households (id, name, timezone, created_at) VALUES (?, 'H', 'America/Chicago', 0)")
            .bind(&hh).execute(&pool).await.unwrap();
        std::mem::forget(dir);
        (pool, hh)
    }

    fn draft(kind: InsightKind, entity: Option<&str>, msg: &str) -> InsightDraft {
        InsightDraft {
            kind,
            entity_id: entity.map(|s| s.to_string()),
            user_message: msg.to_string(),
            extra: None,
        }
    }

    #[tokio::test]
    async fn first_log_returns_some() {
        let (pool, hh) = test_pool().await;
        let result = log_if_new(
            &pool,
            &hh,
            "America/Chicago",
            1_716_000_000_000,
            draft(InsightKind::EnvelopeOver, Some("env1"), "Groceries over budget"),
        )
        .await
        .unwrap();
        assert!(result.is_some());
        let insight = result.unwrap();
        assert_eq!(insight.kind, InsightKind::EnvelopeOver);
        assert_eq!(insight.category, InsightCategory::Alert);
    }

    #[tokio::test]
    async fn second_log_same_day_returns_none() {
        let (pool, hh) = test_pool().await;
        let now = 1_716_000_000_000;
        let _ = log_if_new(&pool, &hh, "America/Chicago", now, draft(InsightKind::EnvelopeOver, Some("env1"), "first")).await.unwrap();
        let second = log_if_new(&pool, &hh, "America/Chicago", now + 1000, draft(InsightKind::EnvelopeOver, Some("env1"), "second")).await.unwrap();
        assert!(second.is_none(), "same day same entity should dedup");
    }

    #[tokio::test]
    async fn different_entity_same_day_logs_separately() {
        let (pool, hh) = test_pool().await;
        let now = 1_716_000_000_000;
        let a = log_if_new(&pool, &hh, "America/Chicago", now, draft(InsightKind::EnvelopeOver, Some("env1"), "a")).await.unwrap();
        let b = log_if_new(&pool, &hh, "America/Chicago", now, draft(InsightKind::EnvelopeOver, Some("env2"), "b")).await.unwrap();
        assert!(a.is_some());
        assert!(b.is_some());
    }

    #[tokio::test]
    async fn different_kind_same_entity_logs_separately() {
        let (pool, hh) = test_pool().await;
        let now = 1_716_000_000_000;
        let a = log_if_new(&pool, &hh, "America/Chicago", now, draft(InsightKind::EnvelopeOver, Some("env1"), "a")).await.unwrap();
        let b = log_if_new(&pool, &hh, "America/Chicago", now, draft(InsightKind::EnvelopeApproaching, Some("env1"), "b")).await.unwrap();
        assert!(a.is_some());
        assert!(b.is_some());
    }

    #[tokio::test]
    async fn different_day_same_entity_logs_separately() {
        let (pool, hh) = test_pool().await;
        let day1 = 1_716_000_000_000; // 2024-05-18 some time
        let day2 = day1 + 86_400_000 * 2; // two days later, definitively a new day in Chicago
        let a = log_if_new(&pool, &hh, "America/Chicago", day1, draft(InsightKind::EnvelopeOver, Some("env1"), "day1")).await.unwrap();
        let b = log_if_new(&pool, &hh, "America/Chicago", day2, draft(InsightKind::EnvelopeOver, Some("env1"), "day2")).await.unwrap();
        assert!(a.is_some());
        assert!(b.is_some());
    }

    #[tokio::test]
    async fn singleton_dedups_within_day_via_partial_index() {
        // Briefing has no entity_id; partial index ensures one per day.
        let (pool, hh) = test_pool().await;
        let now = 1_716_000_000_000;
        let a = log_if_new(&pool, &hh, "America/Chicago", now, draft(InsightKind::MorningBriefing, None, "morning a")).await.unwrap();
        let b = log_if_new(&pool, &hh, "America/Chicago", now + 3600_000, draft(InsightKind::MorningBriefing, None, "morning b")).await.unwrap();
        assert!(a.is_some());
        assert!(b.is_none(), "morning briefing must be once per day");
    }

    #[test]
    fn should_emit_quiet_blocks_everything() {
        for kind in [InsightKind::EnvelopeOver, InsightKind::EnvelopeApproaching, InsightKind::PossibleDuplicate, InsightKind::MorningBriefing] {
            assert!(!should_emit(kind, Sensitivity::Quiet), "{kind:?} should be blocked at quiet");
        }
    }

    #[test]
    fn should_emit_normal_allows_alerts_only() {
        assert!(should_emit(InsightKind::EnvelopeOver, Sensitivity::Normal));
        assert!(should_emit(InsightKind::PossibleDuplicate, Sensitivity::Normal));
        assert!(!should_emit(InsightKind::EnvelopeApproaching, Sensitivity::Normal));
        assert!(!should_emit(InsightKind::MorningBriefing, Sensitivity::Normal));
    }

    #[test]
    fn should_emit_proactive_allows_everything() {
        for kind in [InsightKind::EnvelopeOver, InsightKind::EnvelopeApproaching, InsightKind::PossibleDuplicate, InsightKind::MorningBriefing] {
            assert!(should_emit(kind, Sensitivity::Proactive), "{kind:?} should be allowed at proactive");
        }
    }

    #[test]
    fn day_bucket_uses_household_local_midnight() {
        // 2024-05-18 23:00 Chicago = 2024-05-19 04:00 UTC. Bucket should be
        // the Chicago midnight, not UTC midnight.
        let chicago_2024_05_18_2300 = chrono_tz::America::Chicago
            .with_ymd_and_hms(2024, 5, 18, 23, 0, 0)
            .unwrap()
            .timestamp_millis();
        let bucket = day_bucket_ms("America/Chicago", chicago_2024_05_18_2300).unwrap();
        let expected = chrono_tz::America::Chicago
            .with_ymd_and_hms(2024, 5, 18, 0, 0, 0)
            .unwrap()
            .timestamp_millis();
        assert_eq!(bucket, expected);
    }

    #[test]
    fn category_mapping_matches_spec() {
        assert_eq!(InsightKind::EnvelopeOver.category(), InsightCategory::Alert);
        assert_eq!(InsightKind::EnvelopeApproaching.category(), InsightCategory::Insight);
        assert_eq!(InsightKind::PossibleDuplicate.category(), InsightCategory::Insight);
        assert_eq!(InsightKind::MorningBriefing.category(), InsightCategory::Briefing);
    }

    #[test]
    fn sensitivity_parse_round_trip() {
        for s in ["quiet", "normal", "proactive"] {
            let parsed = Sensitivity::parse(s).unwrap();
            assert_eq!(parsed.as_str(), s);
        }
        assert!(Sensitivity::parse("loud").is_none());
    }
}
