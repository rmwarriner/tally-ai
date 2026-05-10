// Backend adapters — T-020
// Trait definition + claude.rs implementation; Phase 2 adds GPT/Gemini/Ollama.
pub mod claude;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ai::BuiltPrompt;
use crate::core::proposal::TransactionProposal;

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Claude did not return a tool use block")]
    NoToolUse,
    #[error("Failed to parse tool input: {0}")]
    ParseError(String),
    #[error("Claude API error {status}: {message}")]
    ApiError { status: u16, message: String },
}

/// Token usage reported by the AI backend (T-069). Mirrors the columns on
/// `transactions` written by `commit_proposal`. Adapters that can't report
/// usage (mocks, future local-LLM) return `AiUsage::default()`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AiUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    /// True when Anthropic reported any `cache_read_input_tokens` for the call.
    pub cache_hit: bool,
}

/// Adapter return: the parsed proposal plus the token usage from the call.
#[derive(Debug, Clone)]
pub struct ProposeResult {
    pub proposal: TransactionProposal,
    pub usage: AiUsage,
}

#[async_trait]
pub trait AiAdapter: Send + Sync {
    async fn propose(&self, prompt: &BuiltPrompt) -> Result<ProposeResult, AdapterError>;
}
