// Claude Anthropic adapter — T-020
// Uses tool use for TransactionProposal; two-pass fallback on tool use failure (T-026).
// Model: claude-sonnet-4-5 per spec Section 2.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{AdapterError, AiAdapter, Message, Role};
use crate::ai::parser::{self, ClaudeResponse};
use crate::core::proposal::TransactionProposal;

const MODEL: &str = "claude-sonnet-4-5";
const MAX_TOKENS: u32 = 1024;
const API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct ClaudeAdapter {
    api_key: String,
    client: Client,
}

impl ClaudeAdapter {
    pub fn new(api_key: String) -> Self {
        Self { api_key, client: Client::new() }
    }

    pub fn proposal_tool() -> Value {
        json!({
            "name": parser::TOOL_NAME,
            "description": "Submit a structured transaction proposal for validation and posting.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "memo": {
                        "type": "string",
                        "description": "Optional memo describing the transaction."
                    },
                    "txn_date_ms": {
                        "type": "integer",
                        "description": "Unix milliseconds — UTC midnight of the local transaction date."
                    },
                    "lines": {
                        "type": "array",
                        "description": "Journal lines. Debits and credits must balance.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "account_id": { "type": "string" },
                                "envelope_id": { "type": "string" },
                                "amount_cents": {
                                    "type": "integer",
                                    "description": "Always positive; direction encoded in side."
                                },
                                "side": {
                                    "type": "string",
                                    "enum": ["debit", "credit"]
                                }
                            },
                            "required": ["account_id", "amount_cents", "side"]
                        }
                    }
                },
                "required": ["txn_date_ms", "lines"]
            }
        })
    }
}

#[derive(Serialize)]
struct RequestBody {
    model: &'static str,
    max_tokens: u32,
    messages: Vec<RequestMessage>,
    tools: Vec<Value>,
    tool_choice: ToolChoice,
}

#[derive(Serialize)]
struct RequestMessage {
    role: &'static str,
    content: String,
}

#[derive(Serialize)]
struct ToolChoice {
    #[serde(rename = "type")]
    kind: &'static str,
    name: &'static str,
}

#[derive(Deserialize)]
struct ApiErrorBody {
    error: ApiErrorDetail,
}

#[derive(Deserialize)]
struct ApiErrorDetail {
    message: String,
}

#[async_trait]
impl AiAdapter for ClaudeAdapter {
    async fn propose(&self, messages: &[Message]) -> Result<TransactionProposal, AdapterError> {
        let request_messages = messages
            .iter()
            .map(|m| RequestMessage {
                role: match m.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                },
                content: m.content.clone(),
            })
            .collect();

        let body = RequestBody {
            model: MODEL,
            max_tokens: MAX_TOKENS,
            messages: request_messages,
            tools: vec![Self::proposal_tool()],
            tool_choice: ToolChoice { kind: "tool", name: parser::TOOL_NAME },
        };

        let resp = self
            .client
            .post(API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let message = resp
                .json::<ApiErrorBody>()
                .await
                .map(|b| b.error.message)
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(AdapterError::ApiError { status: status.as_u16(), message });
        }

        let claude_resp: ClaudeResponse = resp.json().await?;
        parser::extract_proposal(&claude_resp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposal_tool_has_correct_name() {
        let tool = ClaudeAdapter::proposal_tool();
        assert_eq!(tool["name"], parser::TOOL_NAME);
    }

    #[test]
    fn proposal_tool_schema_requires_txn_date_ms_and_lines() {
        let tool = ClaudeAdapter::proposal_tool();
        let required = &tool["input_schema"]["required"];
        assert!(required.as_array().unwrap().contains(&json!("txn_date_ms")));
        assert!(required.as_array().unwrap().contains(&json!("lines")));
    }

    #[test]
    fn proposal_tool_schema_does_not_require_memo() {
        let tool = ClaudeAdapter::proposal_tool();
        let required = tool["input_schema"]["required"].as_array().unwrap();
        assert!(!required.contains(&json!("memo")));
    }

    #[test]
    fn proposal_tool_line_schema_requires_account_amount_side() {
        let tool = ClaudeAdapter::proposal_tool();
        let line_required =
            tool["input_schema"]["properties"]["lines"]["items"]["required"].as_array().unwrap();
        assert!(line_required.contains(&json!("account_id")));
        assert!(line_required.contains(&json!("amount_cents")));
        assert!(line_required.contains(&json!("side")));
    }

    #[test]
    fn proposal_tool_side_enum_contains_debit_and_credit() {
        let tool = ClaudeAdapter::proposal_tool();
        let side_enum = tool["input_schema"]["properties"]["lines"]["items"]["properties"]["side"]
            ["enum"]
            .as_array()
            .unwrap();
        assert!(side_enum.contains(&json!("debit")));
        assert!(side_enum.contains(&json!("credit")));
    }

    #[test]
    fn request_body_serializes_tool_choice_as_forced() {
        let tool_choice = ToolChoice { kind: "tool", name: parser::TOOL_NAME };
        let json = serde_json::to_value(&tool_choice).unwrap();
        assert_eq!(json["type"], "tool");
        assert_eq!(json["name"], parser::TOOL_NAME);
    }

    #[test]
    fn message_helper_sets_correct_role() {
        let user = Message::user("hello");
        let asst = Message::assistant("hi");
        assert_eq!(user.role, Role::User);
        assert_eq!(asst.role, Role::Assistant);
        assert_eq!(user.content, "hello");
        assert_eq!(asst.content, "hi");
    }
}
