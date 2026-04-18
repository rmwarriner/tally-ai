// Backend adapters — T-020
// Trait definition + claude.rs implementation; Phase 2 adds GPT/Gemini/Ollama.
pub mod claude;

use async_trait::async_trait;
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

#[async_trait]
pub trait AiAdapter: Send + Sync {
    async fn propose(&self, prompt: &BuiltPrompt) -> Result<TransactionProposal, AdapterError>;
}
