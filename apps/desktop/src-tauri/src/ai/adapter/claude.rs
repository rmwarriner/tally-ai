// Claude Anthropic adapter — T-020 / T-026
// Uses tool use for TransactionProposal (T-020).
// On NoToolUse, retries with explicit JSON schema in the system prompt (T-026).
// Model: claude-sonnet-4-5 per spec Section 2.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{AdapterError, AiAdapter, AiUsage, ProposeResult};
use crate::ai::parser::{self, ClaudeResponse};
use crate::ai::{BuiltPrompt, Message, Role};
#[cfg(test)]
use crate::ai::parser::UsageBlock;

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

    /// Compact tool schema (T-070). Prose descriptions removed where the
    /// property name + type are self-documenting; only fields whose semantics
    /// are non-obvious carry a description. The serialized JSON is
    /// length-gated by `tool_definition_under_token_budget`.
    pub fn proposal_tool() -> Value {
        json!({
            "name": parser::TOOL_NAME,
            "description": "Submit a transaction proposal for validation and posting.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "memo": { "type": "string" },
                    "txn_date_ms": {
                        "type": "integer",
                        "description": "UTC midnight of the local txn date, unix ms."
                    },
                    "lines": {
                        "type": "array",
                        "description": "Debits and credits must balance.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "account_id": { "type": "string" },
                                "envelope_id": { "type": "string" },
                                "amount_cents": { "type": "integer" },
                                "side": { "type": "string", "enum": ["debit", "credit"] }
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
    ) -> Result<ProposeResult, AdapterError> {
        let body = ToolUseRequest {
            model: MODEL,
            max_tokens: MAX_TOKENS,
            // T-066: send `system` as content blocks so we can mark the BASE
            // chunk ephemeral. Anthropic's prompt cache (5-min TTL) collapses
            // ~600 cached tokens to ~10% billing on every subsequent request
            // in the session.
            system: cached_system_blocks(&prompt.system),
            messages: to_request_messages(&prompt.messages),
            tools: vec![cached_tool_block(Self::proposal_tool())],
            tool_choice: ToolChoice { kind: "tool", name: parser::TOOL_NAME },
        };

        let resp = self.send_request(&body).await?;
        let claude_resp: ClaudeResponse = resp.json().await?;
        let proposal = parser::extract_proposal(&claude_resp)?;
        let usage = usage_from(&claude_resp);
        Ok(ProposeResult { proposal, usage })
    }

    async fn call_json_fallback(
        &self,
        prompt: &BuiltPrompt,
    ) -> Result<ProposeResult, AdapterError> {
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
        let proposal = match text {
            Some(t) => parser::extract_proposal_from_text(t)?,
            None => return Err(AdapterError::NoToolUse),
        };
        let usage = usage_from(&claude_resp);
        Ok(ProposeResult { proposal, usage })
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

/// Splits `system` into a single content block carrying `cache_control`.
/// We send the full prompt as one chunk because BASE + SNAPSHOT both stay
/// stable within a chat session (snapshot updates after every commit, but
/// reads dominate writes by far).
fn cached_system_blocks(system: &str) -> Vec<Value> {
    vec![json!({
        "type": "text",
        "text": system,
        "cache_control": { "type": "ephemeral" },
    })]
}

/// Wraps the tool definition with a `cache_control` marker. Tool definitions
/// are static for the life of the binary so a cache hit is the common case.
fn cached_tool_block(mut tool: Value) -> Value {
    if let Some(map) = tool.as_object_mut() {
        map.insert(
            "cache_control".to_string(),
            json!({ "type": "ephemeral" }),
        );
    }
    tool
}

/// Folds the parser's `usage` block into the trait-level `AiUsage`. A
/// missing usage object (older fixtures, fallback path) becomes zeros so
/// callers never have to special-case `None`.
fn usage_from(resp: &ClaudeResponse) -> AiUsage {
    let u = resp.usage.clone().unwrap_or_default();
    AiUsage {
        input_tokens: u.input_tokens,
        output_tokens: u.output_tokens,
        cache_hit: u.cache_read_input_tokens > 0,
    }
}

#[cfg(test)]
mod usage_tests {
    use super::*;

    fn resp_with(usage: Option<UsageBlock>) -> ClaudeResponse {
        ClaudeResponse { content: vec![], stop_reason: None, usage }
    }

    #[test]
    fn usage_zeroed_when_block_missing() {
        let u = usage_from(&resp_with(None));
        assert_eq!(u.input_tokens, 0);
        assert_eq!(u.output_tokens, 0);
        assert!(!u.cache_hit);
    }

    #[test]
    fn cache_hit_true_when_cache_read_tokens_nonzero() {
        let u = usage_from(&resp_with(Some(UsageBlock {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 600,
        })));
        assert_eq!(u.input_tokens, 100);
        assert_eq!(u.output_tokens, 50);
        assert!(u.cache_hit);
    }

    #[test]
    fn cache_hit_false_when_only_cache_creation_present() {
        // First call in a session writes the cache but doesn't read it yet.
        let u = usage_from(&resp_with(Some(UsageBlock {
            input_tokens: 700,
            output_tokens: 50,
            cache_creation_input_tokens: 600,
            cache_read_input_tokens: 0,
        })));
        assert!(!u.cache_hit);
    }
}

#[derive(Serialize)]
struct ToolUseRequest {
    model: &'static str,
    max_tokens: u32,
    system: Vec<Value>,
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
    async fn propose(&self, prompt: &BuiltPrompt) -> Result<ProposeResult, AdapterError> {
        match self.call_tool_use(prompt).await {
            Ok(result) => Ok(result),
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

    /// T-066: BASE + SNAPSHOT system content must carry an `ephemeral`
    /// cache_control marker so the cache covers it. Without this the
    /// 30–40% per-message savings disappear silently.
    #[test]
    fn cached_system_blocks_carry_ephemeral_marker() {
        let blocks = cached_system_blocks("system text");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["type"], "text");
        assert_eq!(blocks[0]["text"], "system text");
        assert_eq!(blocks[0]["cache_control"]["type"], "ephemeral");
    }

    /// T-066: tool definition must also be cached. Same rationale; the
    /// tool block is static across the binary's lifetime.
    #[test]
    fn cached_tool_block_carries_ephemeral_marker() {
        let block = cached_tool_block(ClaudeAdapter::proposal_tool());
        assert_eq!(block["cache_control"]["type"], "ephemeral");
        // Make sure the wrap didn't drop the schema fields.
        assert_eq!(block["name"], parser::TOOL_NAME);
        assert!(block["input_schema"]["properties"]["lines"].is_object());
    }

    /// T-070 gate. The tool definition is sent on every API call and
    /// compounds across the session. The 350-token budget keeps the static
    /// per-call cost in check; we approximate Anthropic's tokenizer with
    /// the same `chars / 4` rule used in `ai::prompt::approx_tokens`.
    /// Budget = 350 tokens × 4 chars/token = 1400 chars.
    #[test]
    fn tool_definition_under_token_budget() {
        const MAX_CHARS: usize = 1400;
        let serialized = serde_json::to_string(&ClaudeAdapter::proposal_tool()).unwrap();
        assert!(
            serialized.len() <= MAX_CHARS,
            "tool definition is {} chars (≈{} tokens); budget is {} chars (≈{} tokens). \
             Trim descriptions or split rather than expand the budget.",
            serialized.len(),
            serialized.len() / 4,
            MAX_CHARS,
            MAX_CHARS / 4,
        );
    }
}
