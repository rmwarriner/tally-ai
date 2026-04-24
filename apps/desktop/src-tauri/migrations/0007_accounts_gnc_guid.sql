-- Add gnc_guid column to accounts so reconciliation can match Tally rows to
-- GnuCash source rows unambiguously. Leaf-name matching was unsound for
-- books with repeated leaf names under different parent paths.

ALTER TABLE accounts ADD COLUMN gnc_guid TEXT;
CREATE INDEX idx_accounts_gnc_guid ON accounts(gnc_guid) WHERE gnc_guid IS NOT NULL;
