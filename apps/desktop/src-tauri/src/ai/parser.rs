// TransactionProposal parser — T-020
// Extracts typed proposal from Claude tool-use response; never parses free-form text.

use serde::Deserialize;
use serde_json::Value;

use crate::ai::adapter::AdapterError;
use crate::core::proposal::TransactionProposal;

pub const TOOL_NAME: &str = "submit_transaction_proposal";

#[derive(Debug, Deserialize)]
pub struct ClaudeResponse {
    pub content: Vec<ContentBlock>,
    pub stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text { text: String },
    ToolUse { id: String, name: String, input: Value },
}

pub fn extract_proposal(response: &ClaudeResponse) -> Result<TransactionProposal, AdapterError> {
    let input = response
        .content
        .iter()
        .find_map(|block| match block {
            ContentBlock::ToolUse { name, input, .. } if name == TOOL_NAME => Some(input),
            _ => None,
        })
        .ok_or(AdapterError::NoToolUse)?;

    serde_json::from_value(input.clone()).map_err(|e| AdapterError::ParseError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn valid_response() -> ClaudeResponse {
        ClaudeResponse {
            stop_reason: Some("tool_use".to_string()),
            content: vec![ContentBlock::ToolUse {
                id: "tu_01abc".to_string(),
                name: TOOL_NAME.to_string(),
                input: json!({
                    "txn_date_ms": 1_700_000_000_000_i64,
                    "lines": [
                        { "account_id": "acc_groceries", "amount_cents": 4250, "side": "debit" },
                        { "account_id": "acc_checking",  "amount_cents": 4250, "side": "credit" }
                    ]
                }),
            }],
        }
    }

    #[test]
    fn extracts_proposal_from_valid_tool_use() {
        let proposal = extract_proposal(&valid_response()).unwrap();
        assert_eq!(proposal.txn_date_ms, 1_700_000_000_000);
        assert_eq!(proposal.lines.len(), 2);
        assert!(proposal.memo.is_none());
    }

    #[test]
    fn extracts_memo_when_present() {
        let mut resp = valid_response();
        if let ContentBlock::ToolUse { input, .. } = &mut resp.content[0] {
            input["memo"] = json!("Whole Foods");
        }
        let proposal = extract_proposal(&resp).unwrap();
        assert_eq!(proposal.memo.as_deref(), Some("Whole Foods"));
    }

    #[test]
    fn errors_when_no_tool_use_block() {
        let resp = ClaudeResponse {
            stop_reason: Some("end_turn".to_string()),
            content: vec![ContentBlock::Text {
                text: "I can help with that transaction.".to_string(),
            }],
        };
        assert!(matches!(extract_proposal(&resp), Err(AdapterError::NoToolUse)));
    }

    #[test]
    fn errors_when_tool_name_does_not_match() {
        let resp = ClaudeResponse {
            stop_reason: Some("tool_use".to_string()),
            content: vec![ContentBlock::ToolUse {
                id: "tu_01".to_string(),
                name: "some_other_tool".to_string(),
                input: json!({}),
            }],
        };
        assert!(matches!(extract_proposal(&resp), Err(AdapterError::NoToolUse)));
    }

    #[test]
    fn errors_on_missing_required_field() {
        let resp = ClaudeResponse {
            stop_reason: Some("tool_use".to_string()),
            content: vec![ContentBlock::ToolUse {
                id: "tu_01".to_string(),
                name: TOOL_NAME.to_string(),
                input: json!({ "memo": "no date or lines" }),
            }],
        };
        assert!(matches!(extract_proposal(&resp), Err(AdapterError::ParseError(_))));
    }

    #[test]
    fn errors_on_invalid_input_type() {
        let resp = ClaudeResponse {
            stop_reason: Some("tool_use".to_string()),
            content: vec![ContentBlock::ToolUse {
                id: "tu_01".to_string(),
                name: TOOL_NAME.to_string(),
                input: json!("not an object"),
            }],
        };
        assert!(matches!(extract_proposal(&resp), Err(AdapterError::ParseError(_))));
    }

    #[test]
    fn picks_correct_tool_when_multiple_blocks_present() {
        let resp = ClaudeResponse {
            stop_reason: Some("tool_use".to_string()),
            content: vec![
                ContentBlock::Text { text: "Sure, let me record that.".to_string() },
                ContentBlock::ToolUse {
                    id: "tu_01".to_string(),
                    name: TOOL_NAME.to_string(),
                    input: json!({
                        "txn_date_ms": 1_700_000_000_000_i64,
                        "lines": [
                            { "account_id": "acc_a", "amount_cents": 100, "side": "debit" },
                            { "account_id": "acc_b", "amount_cents": 100, "side": "credit" }
                        ]
                    }),
                },
            ],
        };
        let proposal = extract_proposal(&resp).unwrap();
        assert_eq!(proposal.lines.len(), 2);
    }

    #[test]
    fn preserves_envelope_id_on_lines() {
        let resp = ClaudeResponse {
            stop_reason: Some("tool_use".to_string()),
            content: vec![ContentBlock::ToolUse {
                id: "tu_01".to_string(),
                name: TOOL_NAME.to_string(),
                input: json!({
                    "txn_date_ms": 1_700_000_000_000_i64,
                    "lines": [{
                        "account_id": "acc_groceries",
                        "envelope_id": "env_food",
                        "amount_cents": 5000,
                        "side": "debit"
                    }, {
                        "account_id": "acc_checking",
                        "amount_cents": 5000,
                        "side": "credit"
                    }]
                }),
            }],
        };
        let proposal = extract_proposal(&resp).unwrap();
        assert_eq!(proposal.lines[0].envelope_id.as_deref(), Some("env_food"));
        assert!(proposal.lines[1].envelope_id.is_none());
    }
}
