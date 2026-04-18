-- Session summary compression — T-027
-- Stores rolling compressed summaries per household session.
-- Rows older than 12 months are pruned on each write.
CREATE TABLE session_summaries (
    id           TEXT PRIMARY KEY NOT NULL,
    household_id TEXT NOT NULL REFERENCES households(id),
    session_id   TEXT NOT NULL,
    summary_text TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_session_summaries_household_date
    ON session_summaries(household_id, created_at_ms DESC);
