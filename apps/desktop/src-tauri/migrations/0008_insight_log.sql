-- Insight log (T-053): durable record of proactive messages emitted to the
-- user. Backs every proactive engine producer (envelope alerts, duplicate
-- detection, morning briefing) so we can dedup repeated triggers and
-- support a future GC pass.
--
-- Dedup key: (household_id, kind, entity_id, day_bucket). One alert per
-- envelope per day; one duplicate alert per (payee, amount, txn_date)
-- tuple. day_bucket is the unix-millis midnight of the household-local
-- day on which the insight was emitted (caller computes via chrono-tz).

CREATE TABLE insight_log (
    id            TEXT PRIMARY KEY,            -- ULID
    household_id  TEXT NOT NULL,
    kind          TEXT NOT NULL,               -- envelope_over | envelope_approaching | possible_duplicate | morning_briefing
    entity_id     TEXT,                        -- envelope_id, txn_id, etc; NULL for kind-level singletons (briefing)
    day_bucket    INTEGER NOT NULL,            -- household-local midnight unix-ms of emit day
    payload       TEXT NOT NULL,               -- JSON: user_message + any kind-specific fields
    created_at    INTEGER NOT NULL,            -- unix-ms
    FOREIGN KEY (household_id) REFERENCES households(id)
);

-- Dedup invariant. NULL entity_id participates in the unique key (SQLite
-- treats NULLs as distinct in UNIQUE indexes, so kind-level singletons
-- like morning_briefing get one row per (household, kind, day_bucket)
-- via the partial index below — the main UNIQUE handles entity-scoped
-- kinds).
CREATE UNIQUE INDEX idx_insight_log_dedup_entity
    ON insight_log (household_id, kind, entity_id, day_bucket)
    WHERE entity_id IS NOT NULL;

-- Singletons (no entity_id): one per (household, kind, day_bucket).
CREATE UNIQUE INDEX idx_insight_log_dedup_singleton
    ON insight_log (household_id, kind, day_bucket)
    WHERE entity_id IS NULL;

-- Cheap GC + recency lookups (briefing assembly reads recent insights).
CREATE INDEX idx_insight_log_household_created
    ON insight_log (household_id, created_at DESC);
