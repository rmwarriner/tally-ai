-- Phase 1 Initial Schema for Tally.ai
-- All monetary amounts stored as INTEGER cents (never REAL/FLOAT)
-- All dates stored as unix milliseconds UTC midnight of local date
-- All primary keys are ULIDs stored as TEXT

-- Core identity tables
CREATE TABLE households (
    id             TEXT PRIMARY KEY NOT NULL,
    name           TEXT NOT NULL,
    timezone       TEXT NOT NULL,  -- IANA timezone name (e.g. America/Chicago), no default
    schema_version INTEGER NOT NULL DEFAULT 1,
    created_at     INTEGER NOT NULL  -- unix ms
);

CREATE TABLE users (
    id             TEXT PRIMARY KEY NOT NULL,
    household_id   TEXT NOT NULL REFERENCES households(id),
    display_name   TEXT NOT NULL,
    role           TEXT NOT NULL CHECK(role IN ('owner', 'member')),
    is_active      BOOLEAN NOT NULL DEFAULT 1,
    created_at     INTEGER NOT NULL
);

-- Hierarchical chart of accounts
CREATE TABLE accounts (
    id              TEXT PRIMARY KEY NOT NULL,
    household_id    TEXT NOT NULL REFERENCES households(id),
    parent_id       TEXT REFERENCES accounts(id),
    name            TEXT NOT NULL,
    type            TEXT NOT NULL CHECK(type IN ('asset','liability','income','expense','equity')),
    normal_balance  TEXT NOT NULL CHECK(normal_balance IN ('debit','credit')),
    is_placeholder  BOOLEAN NOT NULL DEFAULT 0,  -- true = grouping only, no journal lines
    currency        TEXT NOT NULL DEFAULT 'USD',
    created_at      INTEGER NOT NULL
);

-- Transaction headers
CREATE TABLE transactions (
    id              TEXT PRIMARY KEY NOT NULL,
    household_id    TEXT NOT NULL REFERENCES households(id),
    txn_date        INTEGER NOT NULL,  -- unix ms, UTC midnight of local date
    entry_date      INTEGER NOT NULL,  -- when recorded
    status          TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','posted','void')),
    source          TEXT NOT NULL CHECK(source IN ('manual','ai','scheduled','import','opening_balance')),
    memo            TEXT,
    corrects_txn_id TEXT REFERENCES transactions(id),
    ai_confidence   REAL CHECK(ai_confidence >= 0.0 AND ai_confidence <= 1.0),
    ai_prompt_hash  TEXT,  -- SHA-256 of prompt for audit
    import_id       TEXT,
    source_line     TEXT,  -- raw unparsed input, max 4KB
    created_at      INTEGER NOT NULL
);

-- Double-entry journal lines
CREATE TABLE journal_lines (
    id             TEXT PRIMARY KEY NOT NULL,
    transaction_id TEXT NOT NULL REFERENCES transactions(id),
    account_id     TEXT NOT NULL REFERENCES accounts(id),
    envelope_id    TEXT,  -- nullable, expense lines only
    amount         INTEGER NOT NULL CHECK(amount > 0),  -- always positive cents
    side           TEXT NOT NULL CHECK(side IN ('debit','credit')),
    memo           TEXT,
    created_at     INTEGER NOT NULL
);

-- Envelope tracking
CREATE TABLE envelopes (
    id             TEXT PRIMARY KEY NOT NULL,
    household_id   TEXT NOT NULL REFERENCES households(id),
    account_id     TEXT NOT NULL REFERENCES accounts(id),
    name           TEXT NOT NULL,
    created_at     INTEGER NOT NULL
);

CREATE TABLE envelope_periods (
    id             TEXT PRIMARY KEY NOT NULL,
    envelope_id    TEXT NOT NULL REFERENCES envelopes(id),
    period_start   INTEGER NOT NULL,  -- unix ms, UTC midnight of local date
    period_end     INTEGER NOT NULL,  -- unix ms, UTC midnight of local date
    allocated      INTEGER NOT NULL DEFAULT 0 CHECK(allocated >= 0),  -- cents
    spent          INTEGER NOT NULL DEFAULT 0 CHECK(spent >= 0),  -- cents, updated by trigger
    created_at     INTEGER NOT NULL
);

-- INSERT-only audit log
CREATE TABLE audit_log (
    id           TEXT PRIMARY KEY NOT NULL,
    household_id TEXT NOT NULL REFERENCES households(id),
    table_name   TEXT NOT NULL,
    row_id       TEXT NOT NULL,
    action       TEXT NOT NULL CHECK(action IN ('insert','update','delete')),
    payload      TEXT NOT NULL,  -- JSON
    user_id      TEXT REFERENCES users(id),
    created_at   INTEGER NOT NULL
);

-- Indexes for query performance
CREATE INDEX idx_users_household               ON users(household_id);
CREATE INDEX idx_accounts_household            ON accounts(household_id);
CREATE INDEX idx_accounts_parent               ON accounts(parent_id);
CREATE INDEX idx_transactions_household        ON transactions(household_id);
CREATE INDEX idx_transactions_date             ON transactions(txn_date);
CREATE INDEX idx_transactions_status           ON transactions(status);
CREATE INDEX idx_journal_lines_transaction     ON journal_lines(transaction_id);
CREATE INDEX idx_journal_lines_account         ON journal_lines(account_id);
CREATE INDEX idx_journal_lines_envelope        ON journal_lines(envelope_id);
CREATE INDEX idx_envelopes_household           ON envelopes(household_id);
CREATE INDEX idx_envelopes_account             ON envelopes(account_id);
CREATE INDEX idx_envelope_periods_envelope     ON envelope_periods(envelope_id);
CREATE INDEX idx_envelope_periods_date_range   ON envelope_periods(period_start, period_end);
CREATE INDEX idx_audit_log_household           ON audit_log(household_id);
CREATE INDEX idx_audit_log_table_row           ON audit_log(table_name, row_id);
CREATE INDEX idx_audit_log_created             ON audit_log(created_at);

-- Trigger: audit_log is INSERT-only
CREATE TRIGGER audit_log_immutable_update
BEFORE UPDATE ON audit_log
BEGIN
    SELECT RAISE(ABORT, 'audit_log is immutable');
END;

CREATE TRIGGER audit_log_immutable_delete
BEFORE DELETE ON audit_log
BEGIN
    SELECT RAISE(ABORT, 'audit_log is immutable');
END;

-- Trigger: atomically update envelope_periods.spent on journal_lines INSERT
-- Fires when a debit is posted to an expense account with envelope_id
CREATE TRIGGER update_envelope_spent_on_insert
AFTER INSERT ON journal_lines
WHEN NEW.envelope_id IS NOT NULL
BEGIN
    UPDATE envelope_periods
    SET spent = spent + NEW.amount
    WHERE envelope_id = NEW.envelope_id
      AND period_start <= (SELECT txn_date FROM transactions WHERE id = NEW.transaction_id)
      AND period_end >= (SELECT txn_date FROM transactions WHERE id = NEW.transaction_id)
      AND (SELECT status FROM transactions WHERE id = NEW.transaction_id) = 'posted';
END;

-- Trigger: atomically update envelope_periods.spent on journal_lines UPDATE
CREATE TRIGGER update_envelope_spent_on_update
AFTER UPDATE ON journal_lines
WHEN NEW.envelope_id IS NOT NULL
BEGIN
    UPDATE envelope_periods
    SET spent = spent - OLD.amount + NEW.amount
    WHERE envelope_id = NEW.envelope_id
      AND period_start <= (SELECT txn_date FROM transactions WHERE id = NEW.transaction_id)
      AND period_end >= (SELECT txn_date FROM transactions WHERE id = NEW.transaction_id)
      AND (SELECT status FROM transactions WHERE id = NEW.transaction_id) = 'posted';
END;

-- Trigger: atomically update envelope_periods.spent on journal_lines DELETE
CREATE TRIGGER update_envelope_spent_on_delete
AFTER DELETE ON journal_lines
WHEN OLD.envelope_id IS NOT NULL
BEGIN
    UPDATE envelope_periods
    SET spent = spent - OLD.amount
    WHERE envelope_id = OLD.envelope_id
      AND (SELECT status FROM transactions WHERE id = OLD.transaction_id) = 'posted';
END;
