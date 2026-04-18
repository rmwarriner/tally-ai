// TransactionProposal parser — T-020 / T-026
// extract_proposal: typed extraction from Claude tool-use response; never parses free-form text.
// extract_proposal_from_text: T-026 fallback — finds JSON in a text response.

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

/// Extract a TransactionProposal from the tool-use content block of a Claude response.
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

/// T-026 fallback: extract a TransactionProposal from a plain-text Claude response.
/// Looks for a ```json...``` block first, then a raw `{...}` object.
pub fn extract_proposal_from_text(text: &str) -> Result<TransactionProposal, AdapterError> {
    let json_str = find_json_in_text(text).ok_or(AdapterError::NoToolUse)?;
    serde_json::from_str(json_str).map_err(|e| AdapterError::ParseError(e.to_string()))
}

fn find_json_in_text(text: &str) -> Option<&str> {
    // Try ```json ... ``` block.
    if let Some(start) = text.find("```json") {
        let after = &text[start + 7..];
        if let Some(end) = after.find("```") {
            return Some(after[..end].trim());
        }
    }
    // Try ``` ... ``` block (language-untagged).
    if let Some(start) = text.find("```") {
        let after = &text[start + 3..];
        if let Some(end) = after.find("```") {
            let candidate = after[..end].trim();
            if candidate.starts_with('{') {
                return Some(candidate);
            }
        }
    }
    // Last resort: outermost { ... }.
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end > start {
        Some(&text[start..=end])
    } else {
        None
    }
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

    // --- extract_proposal ---

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

    // --- extract_proposal_from_text (T-026) ---

    fn valid_json_proposal() -> &'static str {
        r#"{"txn_date_ms":1700000000000,"lines":[{"account_id":"acc_a","amount_cents":100,"side":"debit"},{"account_id":"acc_b","amount_cents":100,"side":"credit"}]}"#
    }

    #[test]
    fn fallback_extracts_from_json_code_block() {
        let text = format!("Here is the transaction:\n```json\n{}\n```", valid_json_proposal());
        let proposal = extract_proposal_from_text(&text).unwrap();
        assert_eq!(proposal.lines.len(), 2);
    }

    #[test]
    fn fallback_extracts_from_untagged_code_block() {
        let text = format!("```\n{}\n```", valid_json_proposal());
        let proposal = extract_proposal_from_text(&text).unwrap();
        assert_eq!(proposal.txn_date_ms, 1_700_000_000_000);
    }

    #[test]
    fn fallback_extracts_from_raw_json_in_prose() {
        let text = format!("Sure! Here: {} Does that look right?", valid_json_proposal());
        let proposal = extract_proposal_from_text(&text).unwrap();
        assert_eq!(proposal.lines.len(), 2);
    }

    #[test]
    fn fallback_errors_when_no_json_present() {
        let text = "I cannot process that request.";
        assert!(matches!(extract_proposal_from_text(text), Err(AdapterError::NoToolUse)));
    }

    #[test]
    fn fallback_errors_on_invalid_schema() {
        let text = r#"{"not_a_proposal": true}"#;
        assert!(matches!(extract_proposal_from_text(text), Err(AdapterError::ParseError(_))));
    }
}
