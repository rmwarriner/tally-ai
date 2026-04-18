pub mod adapter;
pub mod advisories;
pub mod classifier;
pub mod parser;
pub mod payee_memory;
pub mod prompt;
pub mod snapshot;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

/// Assembled prompt ready to send to the AI adapter.
/// Produced by `prompt::PromptBuilder`; consumed by `adapter::AiAdapter`.
#[derive(Debug, Clone)]
pub struct BuiltPrompt {
    /// BASE + SNAPSHOT layers — passed as Claude's `system` parameter; never trimmed.
    pub system: String,
    /// INTENT + trimmed HISTORY + MEMORY layers.
    pub messages: Vec<Message>,
}
