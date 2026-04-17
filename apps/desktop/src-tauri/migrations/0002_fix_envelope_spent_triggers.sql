-- Fix envelope_periods.spent triggers to correctly handle debit vs credit lines.
-- The original triggers always added amount to spent, which incorrectly increased
-- spent for credit (refund) lines. A debit increases spending; a credit decreases it.

DROP TRIGGER IF EXISTS update_envelope_spent_on_insert;
DROP TRIGGER IF EXISTS update_envelope_spent_on_update;
DROP TRIGGER IF EXISTS update_envelope_spent_on_delete;

CREATE TRIGGER update_envelope_spent_on_insert
AFTER INSERT ON journal_lines
WHEN NEW.envelope_id IS NOT NULL
BEGIN
    UPDATE envelope_periods
    SET spent = spent + (CASE WHEN NEW.side = 'debit' THEN NEW.amount ELSE -NEW.amount END)
    WHERE envelope_id = NEW.envelope_id
      AND period_start <= (SELECT txn_date FROM transactions WHERE id = NEW.transaction_id)
      AND period_end   >= (SELECT txn_date FROM transactions WHERE id = NEW.transaction_id)
      AND (SELECT status FROM transactions WHERE id = NEW.transaction_id) = 'posted';
END;

CREATE TRIGGER update_envelope_spent_on_update
AFTER UPDATE ON journal_lines
WHEN NEW.envelope_id IS NOT NULL
BEGIN
    UPDATE envelope_periods
    SET spent = spent
        - (CASE WHEN OLD.side = 'debit' THEN OLD.amount ELSE -OLD.amount END)
        + (CASE WHEN NEW.side = 'debit' THEN NEW.amount ELSE -NEW.amount END)
    WHERE envelope_id = NEW.envelope_id
      AND period_start <= (SELECT txn_date FROM transactions WHERE id = NEW.transaction_id)
      AND period_end   >= (SELECT txn_date FROM transactions WHERE id = NEW.transaction_id)
      AND (SELECT status FROM transactions WHERE id = NEW.transaction_id) = 'posted';
END;

CREATE TRIGGER update_envelope_spent_on_delete
AFTER DELETE ON journal_lines
WHEN OLD.envelope_id IS NOT NULL
BEGIN
    UPDATE envelope_periods
    SET spent = spent - (CASE WHEN OLD.side = 'debit' THEN OLD.amount ELSE -OLD.amount END)
    WHERE envelope_id = OLD.envelope_id
      AND period_start <= (SELECT txn_date FROM transactions WHERE id = OLD.transaction_id)
      AND period_end   >= (SELECT txn_date FROM transactions WHERE id = OLD.transaction_id)
      AND (SELECT status FROM transactions WHERE id = OLD.transaction_id) = 'posted';
END;
