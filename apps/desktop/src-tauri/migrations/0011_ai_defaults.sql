-- Persistent AI defaults table (#144).
--
-- Replaces the hardcoded JSON stub previously returned by get_ai_defaults.
-- Composite primary key on (household_id, key) — each household has its own
-- set of defaults keyed by string. Values are stored as plain text; callers
-- parse as needed (the AI orchestrator validates type at use site).
--
-- Initial keys the codebase reads:
--   default_payment_account  — ULID of the account to default to for new txns
--   default_currency         — currency code (Phase 1 always 'USD')
--   confirm_threshold_cents  — auto-confirm txns at or below this amount

CREATE TABLE IF NOT EXISTS ai_defaults (
    household_id TEXT NOT NULL REFERENCES households(id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (household_id, key)
) STRICT;

CREATE INDEX IF NOT EXISTS idx_ai_defaults_household
    ON ai_defaults(household_id);
