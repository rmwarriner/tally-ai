-- Payee memory — T-024
-- Household-scoped payee → account mappings, updated on every commit.
-- In-process LRU cache (500 entries) sits in front; this table is the durable store.

CREATE TABLE payee_memory (
    id           TEXT PRIMARY KEY NOT NULL,
    household_id TEXT NOT NULL REFERENCES households(id),
    payee_name   TEXT NOT NULL,
    account_id   TEXT NOT NULL REFERENCES accounts(id),
    use_count    INTEGER NOT NULL DEFAULT 1,
    last_used_ms INTEGER NOT NULL,
    created_at   INTEGER NOT NULL
);

-- Case-insensitive uniqueness per household.
CREATE UNIQUE INDEX idx_payee_memory_lookup
    ON payee_memory(household_id, payee_name COLLATE NOCASE);

CREATE INDEX idx_payee_memory_household_count
    ON payee_memory(household_id, use_count DESC);
