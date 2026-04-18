// Claude Anthropic adapter — T-020 / T-026
// Uses tool use for TransactionProposal (T-020).
// On NoToolUse, retries with explicit JSON schema in the system prompt (T-026).
// Model: claude-sonnet-4-5 per spec Section 2.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{AdapterError, AiAdapter};
use crate::ai::parser::{self, ClaudeResponse};
use crate::ai::{BuiltPrompt, Message, Role};
use crate::core::proposal::TransactionProposal;

const MODEL: &str = "claude-sonnet-4-5";
const MAX_TOKENS: u32 = 1024;
const FALLBACK_MAX_TOKENS: u32 = 2048;
const API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Appended to `system` on the second-pass retry so Claude knows to reply with raw JSON.
const FALLBACK_SCHEMA_INSTRUCTION: &str = "\n\n\
    [FALLBACK] Tool use is unavailable for this request. \
    Respond with ONLY a JSON object and nothing else — no prose, no markdown:\n\
    {\n\
      \"txn_date_ms\": <integer, required>,\n\
      \"memo\": \"<string, optional>\",\n\
      \"lines\": [\n\
        {\n\
          \"account_id\": \"<string>\",\n\
          \"amount_cents\": <integer>,\n\
          \"side\": \"debit|credit\",\n\
          \"envelope_id\": \"<string, optional>\"\n\
        }\n\
      ]\n\
    }";

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

    async fn call_tool_use(
        &self,
        prompt: &BuiltPrompt,
    ) -> Result<TransactionProposal, AdapterError> {
        let body = ToolUseRequest {
            model: MODEL,
            max_tokens: MAX_TOKENS,
            system: &prompt.system,
            messages: to_request_messages(&prompt.messages),
            tools: vec![Self::proposal_tool()],
            tool_choice: ToolChoice { kind: "tool", name: parser::TOOL_NAME },
        };

        let resp = self.send_request(&body).await?;
        let claude_resp: ClaudeResponse = resp.json().await?;
        parser::extract_proposal(&claude_resp)
    }

    async fn call_json_fallback(
        &self,
        prompt: &BuiltPrompt,
    ) -> Result<TransactionProposal, AdapterError> {
        let fallback_system = format!("{}{}", prompt.system, FALLBACK_SCHEMA_INSTRUCTION);
        let body = TextRequest {
            model: MODEL,
            max_tokens: FALLBACK_MAX_TOKENS,
            system: &fallback_system,
            messages: to_request_messages(&prompt.messages),
        };

        let resp = self.send_request(&body).await?;
        let claude_resp: ClaudeResponse = resp.json().await?;

        // Extract text content and parse as JSON.
        let text = claude_resp.content.iter().find_map(|b| {
            if let parser::ContentBlock::Text { text } = b { Some(text.as_str()) } else { None }
        });
        match text {
            Some(t) => parser::extract_proposal_from_text(t),
            None => Err(AdapterError::NoToolUse),
        }
    }

    async fn send_request<B: Serialize>(
        &self,
        body: &B,
    ) -> Result<reqwest::Response, AdapterError> {
        let resp = self
            .client
            .post(API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(body)
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

        Ok(resp)
    }
}

fn to_request_messages(messages: &[Message]) -> Vec<RequestMessage> {
    messages
        .iter()
        .map(|m| RequestMessage {
            role: match m.role {
                Role::User => "user",
                Role::Assistant => "assistant",
            },
            content: m.content.clone(),
        })
        .collect()
}

#[derive(Serialize)]
struct ToolUseRequest<'a> {
    model: &'static str,
    max_tokens: u32,
    system: &'a str,
    messages: Vec<RequestMessage>,
    tools: Vec<Value>,
    tool_choice: ToolChoice,
}

#[derive(Serialize)]
struct TextRequest<'a> {
    model: &'static str,
    max_tokens: u32,
    system: &'a str,
    messages: Vec<RequestMessage>,
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
    async fn propose(&self, prompt: &BuiltPrompt) -> Result<TransactionProposal, AdapterError> {
        match self.call_tool_use(prompt).await {
            Ok(proposal) => Ok(proposal),
            // T-026: retry with explicit JSON schema on tool use failure.
            Err(AdapterError::NoToolUse) => self.call_json_fallback(prompt).await,
            Err(e) => Err(e),
        }
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
    fn tool_choice_serializes_as_forced() {
        let tc = ToolChoice { kind: "tool", name: parser::TOOL_NAME };
        let v = serde_json::to_value(&tc).unwrap();
        assert_eq!(v["type"], "tool");
        assert_eq!(v["name"], parser::TOOL_NAME);
    }

    #[test]
    fn message_helpers_set_correct_roles() {
        let user = Message::user("hello");
        let asst = Message::assistant("hi");
        assert_eq!(user.role, Role::User);
        assert_eq!(asst.role, Role::Assistant);
    }

    #[test]
    fn fallback_schema_instruction_is_non_empty() {
        assert!(!FALLBACK_SCHEMA_INSTRUCTION.is_empty());
        assert!(FALLBACK_SCHEMA_INSTRUCTION.contains("txn_date_ms"));
        assert!(FALLBACK_SCHEMA_INSTRUCTION.contains("lines"));
    }

    #[test]
    fn to_request_messages_maps_roles_correctly() {
        let messages = vec![Message::user("u"), Message::assistant("a")];
        let req = to_request_messages(&messages);
        assert_eq!(req[0].role, "user");
        assert_eq!(req[1].role, "assistant");
    }
}
