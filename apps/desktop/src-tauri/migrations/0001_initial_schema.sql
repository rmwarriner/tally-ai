-- Initial schema for Tally.ai Phase 1
-- All monetary amounts stored as INTEGER cents (never REAL/FLOAT)
-- All dates stored as unix milliseconds UTC midnight of local date
-- All primary keys are ULIDs stored as TEXT

CREATE TABLE households (
    id          TEXT PRIMARY KEY NOT NULL,
    name        TEXT NOT NULL,
    timezone    TEXT NOT NULL DEFAULT 'UTC',  -- IANA timezone name
    salt        BLOB NOT NULL,                -- 16-byte Argon2id salt
    created_at  INTEGER NOT NULL              -- unix ms
);

CREATE TABLE accounts (
    id           TEXT PRIMARY KEY NOT NULL,
    household_id TEXT NOT NULL REFERENCES households(id),
    name         TEXT NOT NULL,
    type         TEXT NOT NULL CHECK(type IN ('asset','liability','income','expense','equity')),
    currency     TEXT NOT NULL DEFAULT 'USD',
    created_at   INTEGER NOT NULL
);

CREATE TABLE journal_entries (
    id           TEXT PRIMARY KEY NOT NULL,
    household_id TEXT NOT NULL REFERENCES households(id),
    date         INTEGER NOT NULL,  -- unix ms, UTC midnight of local date
    description  TEXT NOT NULL,
    created_at   INTEGER NOT NULL
);

CREATE TABLE journal_lines (
    id         TEXT PRIMARY KEY NOT NULL,
    entry_id   TEXT NOT NULL REFERENCES journal_entries(id),
    account_id TEXT NOT NULL REFERENCES accounts(id),
    amount     INTEGER NOT NULL CHECK(amount > 0),  -- always positive cents
    side       TEXT NOT NULL CHECK(side IN ('debit','credit')),
    memo       TEXT
);

CREATE TABLE envelopes (
    id             TEXT PRIMARY KEY NOT NULL,
    household_id   TEXT NOT NULL REFERENCES households(id),
    name           TEXT NOT NULL,
    account_id     TEXT REFERENCES accounts(id),  -- optional funding account
    budget_amount  INTEGER NOT NULL DEFAULT 0 CHECK(budget_amount >= 0),  -- cents
    created_at     INTEGER NOT NULL
);

-- INSERT-only: never UPDATE or DELETE rows in this table
CREATE TABLE audit_log (
    id           TEXT PRIMARY KEY NOT NULL,
    household_id TEXT NOT NULL REFERENCES households(id),
    table_name   TEXT NOT NULL,
    row_id       TEXT NOT NULL,
    action       TEXT NOT NULL CHECK(action IN ('insert','update','delete')),
    payload      TEXT NOT NULL,  -- JSON
    created_at   INTEGER NOT NULL
);

CREATE INDEX idx_accounts_household       ON accounts(household_id);
CREATE INDEX idx_journal_entries_household ON journal_entries(household_id);
CREATE INDEX idx_journal_entries_date     ON journal_entries(date);
CREATE INDEX idx_journal_lines_entry      ON journal_lines(entry_id);
CREATE INDEX idx_journal_lines_account    ON journal_lines(account_id);
CREATE INDEX idx_envelopes_household      ON envelopes(household_id);
CREATE INDEX idx_audit_log_household      ON audit_log(household_id);
CREATE INDEX idx_audit_log_row            ON audit_log(table_name, row_id);
