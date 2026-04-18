// Backend adapters — T-020
// Trait definition + claude.rs implementation; Phase 2 adds GPT/Gemini/Ollama.
pub mod claude;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::core::proposal::TransactionProposal;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: Role::User, content: content.into() }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: Role::Assistant, content: content.into() }
    }
}

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
    async fn propose(&self, messages: &[Message]) -> Result<TransactionProposal, AdapterError>;
}
