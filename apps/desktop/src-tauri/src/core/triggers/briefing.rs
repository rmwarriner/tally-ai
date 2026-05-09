// Morning briefing assembler — T-050.
//
// Produces a single proactive insight summarizing the household's state at
// session-open. Items are picked from existing read APIs (no new SQL) and
// capped at 4 to match the spec's "4-item max".
//
// Curation order (highest signal first):
//   1. Total cash on hand (sum of asset accounts).
//   2. Most-spent envelope this period.
//   3. Envelopes over budget — count or "all on track".
//   4. Recent activity in the past 24h — count of posted transactions.
//
// Producers gate by sensitivity (briefing requires `proactive`) and the
// caller writes via `log_if_new` with `entity_id = None` so the singleton
// partial index enforces one per household-local day.

use sqlx::SqlitePool;

use crate::core::insight::{InsightDraft, InsightKind};
use crate::core::read::{account_balances, current_envelope_periods};

const MAX_ITEMS: usize = 4;
const RECENT_WINDOW_MS: i64 = 86_400_000; // 24h

/// Builds the morning briefing draft. Returns `Ok(None)` only on a query
/// failure that the caller should tolerate; an empty briefing is still a
/// briefing (covers fresh accounts with no data).
pub async fn assemble(
    pool: &SqlitePool,
    household_id: &str,
    now_ms: i64,
) -> Result<InsightDraft, sqlx::Error> {
    let mut items: Vec<String> = Vec::with_capacity(MAX_ITEMS);

    // Item 1 — total cash.
    let balances = account_balances(pool, household_id).await?;
    let total_cash: i64 = balances
        .iter()
        .filter(|b| b.account_type == "asset")
        .map(|b| b.balance_cents)
        .sum();
    items.push(format!("Cash on hand: {}.", format_cents(total_cash)));

    // Items 2 + 3 — envelope summary.
    let envelopes = current_envelope_periods(pool, household_id, now_ms).await?;
    if let Some(top) = envelopes
        .iter()
        .filter(|e| e.allocated_cents > 0)
        .max_by_key(|e| e.spent_cents)
    {
        items.push(format!(
            "Top envelope this period: {} ({} of {}).",
            top.name,
            format_cents(top.spent_cents),
            format_cents(top.allocated_cents),
        ));
    }
    let over_count = envelopes
        .iter()
        .filter(|e| e.allocated_cents > 0 && e.spent_cents > e.allocated_cents)
        .count();
    if over_count > 0 {
        let label = if over_count == 1 { "envelope" } else { "envelopes" };
        items.push(format!("{over_count} {label} over budget."));
    } else if !envelopes.is_empty() {
        items.push("All envelopes on track.".to_string());
    }

    // Item 4 — recent activity.
    let since = now_ms - RECENT_WINDOW_MS;
    let (recent_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transactions WHERE household_id = ? AND status = 'posted' AND created_at >= ?",
    )
    .bind(household_id)
    .bind(since)
    .fetch_one(pool)
    .await?;
    let label = if recent_count == 1 { "transaction" } else { "transactions" };
    items.push(format!("{recent_count} {label} in the past 24 hours."));

    items.truncate(MAX_ITEMS);

    let user_message = format!("Good morning! Here's where things stand:\n• {}", items.join("\n• "));

    Ok(InsightDraft {
        kind: InsightKind::MorningBriefing,
        entity_id: None, // singleton — partial index enforces once-per-day
        user_message,
        extra: Some(serde_json::json!({
            "total_cash_cents": total_cash,
            "envelope_count": envelopes.len(),
            "envelopes_over": over_count,
            "recent_24h_count": recent_count,
        })),
    })
}

fn format_cents(cents: i64) -> String {
    let abs = cents.abs();
    let dollars = abs / 100;
    let remainder = abs % 100;
    let sign = if cents < 0 { "-" } else { "" };
    format!("{sign}${dollars}.{remainder:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_encrypted_db;
    use crate::id::new_ulid;
    use tempfile::tempdir;

    async fn pool_with_household() -> (SqlitePool, String) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("brief.db");
        let pool = create_encrypted_db(&path, "pp", &[0u8; 16]).await.unwrap();
        let hh = new_ulid();
        sqlx::query("INSERT INTO households (id, name, timezone, created_at) VALUES (?, 'H', 'America/Chicago', 0)")
            .bind(&hh).execute(&pool).await.unwrap();
        std::mem::forget(dir);
        (pool, hh)
    }

    #[tokio::test]
    async fn empty_household_briefs_with_zero_cash() {
        let (pool, hh) = pool_with_household().await;
        let draft = assemble(&pool, &hh, 1_716_000_000_000).await.unwrap();
        assert_eq!(draft.kind, InsightKind::MorningBriefing);
        assert!(draft.entity_id.is_none(), "briefing is a singleton");
        assert!(draft.user_message.contains("$0.00"));
        assert!(draft.user_message.contains("0 transactions"));
    }

    #[tokio::test]
    async fn briefing_caps_at_four_items() {
        let (pool, hh) = pool_with_household().await;

        // Seed 1 asset account + 3 envelopes (one over budget) so all 4
        // possible items are present and we can confirm the cap.
        sqlx::query("INSERT INTO accounts (id, household_id, name, type, normal_balance, created_at) VALUES ('chk', ?, 'Checking', 'asset', 'debit', 0), ('eq', ?, 'Equity', 'equity', 'credit', 0), ('grc', ?, 'Groceries', 'expense', 'debit', 0), ('gas', ?, 'Gas', 'expense', 'debit', 0)")
            .bind(&hh).bind(&hh).bind(&hh).bind(&hh).execute(&pool).await.unwrap();

        sqlx::query("INSERT INTO envelopes (id, household_id, account_id, name, created_at) VALUES ('e1', ?, 'grc', 'Groceries', 0), ('e2', ?, 'gas', 'Gas', 0)")
            .bind(&hh).bind(&hh).execute(&pool).await.unwrap();

        let now = 1_716_000_000_000;
        let period_start = now - 1_000_000;
        let period_end = now + 86_400_000_i64 * 30;
        sqlx::query("INSERT INTO envelope_periods (id, envelope_id, period_start, period_end, allocated, spent, created_at) VALUES ('p1', 'e1', ?, ?, 50000, 60000, 0), ('p2', 'e2', ?, ?, 10000, 2000, 0)")
            .bind(period_start).bind(period_end).bind(period_start).bind(period_end)
            .execute(&pool).await.unwrap();

        let draft = assemble(&pool, &hh, now).await.unwrap();
        let item_count = draft.user_message.matches('•').count();
        assert!(item_count <= MAX_ITEMS, "got {item_count} items, max {MAX_ITEMS}");
        assert!(draft.user_message.contains("Groceries"));
        assert!(draft.user_message.contains("over budget"));
    }
}
