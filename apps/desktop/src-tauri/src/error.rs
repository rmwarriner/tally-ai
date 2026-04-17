use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecoveryKind {
    CreateMissing,
    UseSuggested,
    EditField,
    PostAnyway,
    Discard,
    ShowHelp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryAction {
    pub kind: RecoveryKind,
    pub label: String,
    pub is_primary: bool,
}

/// NonEmpty wrapper — compile error if constructed with empty Vec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NonEmpty<T>(Vec<T>);

impl<T> NonEmpty<T> {
    pub fn new(first: T, rest: Vec<T>) -> Self {
        let mut v = vec![first];
        v.extend(rest);
        NonEmpty(v)
    }

    pub fn as_slice(&self) -> &[T] {
        &self.0
    }
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Tauri error: {0}")]
    Tauri(String),
}

impl serde::Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
