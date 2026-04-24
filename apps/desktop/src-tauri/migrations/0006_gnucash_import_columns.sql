-- Add GnuCash import support columns:
--   transactions.source_ref  — GnuCash transaction GUID (per-row idempotency)
--   accounts.import_id       — ULID stamped on accounts created by an import (scoped rollback)

ALTER TABLE transactions ADD COLUMN source_ref TEXT;
CREATE INDEX idx_transactions_source_ref
    ON transactions(source_ref) WHERE source_ref IS NOT NULL;
CREATE UNIQUE INDEX idx_transactions_source_ref_unique
    ON transactions(household_id, source_ref) WHERE source_ref IS NOT NULL;

ALTER TABLE accounts ADD COLUMN import_id TEXT;
CREATE INDEX idx_accounts_import_id
    ON accounts(import_id) WHERE import_id IS NOT NULL;
