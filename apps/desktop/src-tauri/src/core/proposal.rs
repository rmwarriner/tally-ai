use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Debit,
    Credit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedLine {
    pub account_id: String,
    pub envelope_id: Option<String>,
    /// Always positive; direction is encoded in `side`.
    pub amount_cents: i64,
    pub side: Side,
}

/// What the AI layer submits for every transaction entry intent.
/// The Rust core validates and commits; the AI layer never writes directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionProposal {
    pub memo: Option<String>,
    /// Unix milliseconds — UTC midnight of the local transaction date.
    pub txn_date_ms: i64,
    pub lines: Vec<ProposedLine>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn debit(account_id: &str, amount_cents: i64) -> ProposedLine {
        ProposedLine {
            account_id: account_id.to_string(),
            envelope_id: None,
            amount_cents,
            side: Side::Debit,
        }
    }

    fn credit(account_id: &str, amount_cents: i64) -> ProposedLine {
        ProposedLine {
            account_id: account_id.to_string(),
            envelope_id: None,
            amount_cents,
            side: Side::Credit,
        }
    }

    #[test]
    fn proposal_roundtrips_json() {
        let proposal = TransactionProposal {
            memo: Some("Grocery run".to_string()),
            txn_date_ms: 1_700_000_000_000,
            lines: vec![
                debit("acc_groceries", 4250),
                credit("acc_checking", 4250),
            ],
        };

        let json = serde_json::to_string(&proposal).expect("serialize");
        let back: TransactionProposal = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(back.memo.as_deref(), Some("Grocery run"));
        assert_eq!(back.txn_date_ms, 1_700_000_000_000);
        assert_eq!(back.lines.len(), 2);
        assert_eq!(back.lines[0].amount_cents, 4250);
        assert_eq!(back.lines[0].side, Side::Debit);
        assert_eq!(back.lines[1].side, Side::Credit);
    }

    #[test]
    fn proposal_memo_is_optional() {
        let proposal = TransactionProposal {
            memo: None,
            txn_date_ms: 1_700_000_000_000,
            lines: vec![debit("acc_a", 100), credit("acc_b", 100)],
        };

        let json = serde_json::to_string(&proposal).expect("serialize");
        let back: TransactionProposal = serde_json::from_str(&json).expect("deserialize");
        assert!(back.memo.is_none());
    }

    #[test]
    fn proposed_line_with_envelope() {
        let line = ProposedLine {
            account_id: "acc_groceries".to_string(),
            envelope_id: Some("env_food".to_string()),
            amount_cents: 5000,
            side: Side::Debit,
        };

        let json = serde_json::to_string(&line).expect("serialize");
        let back: ProposedLine = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.envelope_id.as_deref(), Some("env_food"));
    }

    #[test]
    fn side_serializes_lowercase() {
        let debit_json = serde_json::to_string(&Side::Debit).expect("serialize");
        let credit_json = serde_json::to_string(&Side::Credit).expect("serialize");
        assert_eq!(debit_json, "\"debit\"");
        assert_eq!(credit_json, "\"credit\"");
    }

    #[test]
    fn proposal_can_have_multiple_lines() {
        let proposal = TransactionProposal {
            memo: Some("Split bill".to_string()),
            txn_date_ms: 1_700_000_000_000,
            lines: vec![
                debit("acc_rent", 100_000),
                debit("acc_utilities", 15_000),
                credit("acc_checking", 115_000),
            ],
        };

        assert_eq!(proposal.lines.len(), 3);
        let debit_sum: i64 = proposal
            .lines
            .iter()
            .filter(|l| l.side == Side::Debit)
            .map(|l| l.amount_cents)
            .sum();
        let credit_sum: i64 = proposal
            .lines
            .iter()
            .filter(|l| l.side == Side::Credit)
            .map(|l| l.amount_cents)
            .sum();
        assert_eq!(debit_sum, credit_sum);
    }
}
