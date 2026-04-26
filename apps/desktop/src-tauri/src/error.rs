use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// Guaranteed non-empty list. Construction always requires at least one element;
/// deserialization rejects empty arrays.
#[derive(Debug, Clone, Serialize)]
pub struct NonEmpty<T>(Vec<T>);

impl<T> NonEmpty<T> {
    pub fn new(first: T, rest: Vec<T>) -> Self {
        let mut v = vec![first];
        v.extend(rest);
        NonEmpty(v)
    }

    /// Returns `Some` if `vec` is non-empty, `None` otherwise.
    pub fn from_vec(vec: Vec<T>) -> Option<Self> {
        if vec.is_empty() {
            None
        } else {
            Some(NonEmpty(vec))
        }
    }

    pub fn first(&self) -> &T {
        // Safety: invariant guarantees len >= 1
        &self.0[0]
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.0.iter()
    }

    pub fn as_slice(&self) -> &[T] {
        &self.0
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for NonEmpty<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let vec = Vec::<T>::deserialize(deserializer)?;
        NonEmpty::from_vec(vec).ok_or_else(|| serde::de::Error::custom("NonEmpty cannot be empty"))
    }
}

/// Wire-shape carried by every `Result<T, RecoveryError>` returned from a
/// `#[tauri::command]`. Translated by the frontend `safeInvoke` into a
/// user-facing advisory or an inline error UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryError {
    pub message: String,
    pub recovery: NonEmpty<RecoveryAction>,
}

impl RecoveryError {
    pub fn new(message: impl Into<String>, recovery: NonEmpty<RecoveryAction>) -> Self {
        Self {
            message: message.into(),
            recovery,
        }
    }

    pub fn show_help(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            recovery: NonEmpty::new(
                RecoveryAction {
                    kind: RecoveryKind::ShowHelp,
                    label: "Get help".to_string(),
                    is_primary: true,
                },
                vec![],
            ),
        }
    }

    pub fn discard(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            recovery: NonEmpty::new(
                RecoveryAction {
                    kind: RecoveryKind::Discard,
                    label: "Discard".to_string(),
                    is_primary: true,
                },
                vec![],
            ),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn discard() -> RecoveryAction {
        RecoveryAction {
            kind: RecoveryKind::Discard,
            label: "Discard".to_string(),
            is_primary: false,
        }
    }

    fn edit() -> RecoveryAction {
        RecoveryAction {
            kind: RecoveryKind::EditField,
            label: "Edit".to_string(),
            is_primary: true,
        }
    }

    // -- NonEmpty construction --

    #[test]
    fn nonempty_new_has_correct_len() {
        let ne = NonEmpty::new(discard(), vec![edit()]);
        assert_eq!(ne.len(), 2);
    }

    #[test]
    fn nonempty_first_returns_head() {
        let ne = NonEmpty::new(edit(), vec![discard()]);
        assert_eq!(ne.first().label, "Edit");
    }

    #[test]
    fn nonempty_iter_visits_all_elements() {
        let ne = NonEmpty::new(edit(), vec![discard()]);
        let labels: Vec<&str> = ne.iter().map(|a| a.label.as_str()).collect();
        assert_eq!(labels, vec!["Edit", "Discard"]);
    }

    #[test]
    fn nonempty_from_vec_some_when_nonempty() {
        let result = NonEmpty::from_vec(vec![discard()]);
        assert!(result.is_some());
    }

    #[test]
    fn nonempty_from_vec_none_when_empty() {
        let result: Option<NonEmpty<RecoveryAction>> = NonEmpty::from_vec(vec![]);
        assert!(result.is_none());
    }

    // -- Serialization / deserialization --

    #[test]
    fn nonempty_roundtrips_json() {
        let ne = NonEmpty::new(edit(), vec![discard()]);
        let json = serde_json::to_string(&ne).expect("serialize");
        let back: NonEmpty<RecoveryAction> = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.len(), 2);
        assert_eq!(back.first().label, "Edit");
    }

    #[test]
    fn nonempty_deserialize_rejects_empty_array() {
        let result: Result<NonEmpty<RecoveryAction>, _> = serde_json::from_str("[]");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("NonEmpty cannot be empty"));
    }

    // -- RecoveryKind serialization --

    #[test]
    fn recovery_kind_serializes_screaming_snake_case() {
        assert_eq!(
            serde_json::to_string(&RecoveryKind::CreateMissing).unwrap(),
            "\"CREATE_MISSING\""
        );
        assert_eq!(
            serde_json::to_string(&RecoveryKind::UseSuggested).unwrap(),
            "\"USE_SUGGESTED\""
        );
        assert_eq!(
            serde_json::to_string(&RecoveryKind::EditField).unwrap(),
            "\"EDIT_FIELD\""
        );
        assert_eq!(
            serde_json::to_string(&RecoveryKind::PostAnyway).unwrap(),
            "\"POST_ANYWAY\""
        );
        assert_eq!(
            serde_json::to_string(&RecoveryKind::Discard).unwrap(),
            "\"DISCARD\""
        );
        assert_eq!(
            serde_json::to_string(&RecoveryKind::ShowHelp).unwrap(),
            "\"SHOW_HELP\""
        );
    }

    #[test]
    fn recovery_action_roundtrips_json() {
        let action = RecoveryAction {
            kind: RecoveryKind::CreateMissing,
            label: "Create account".to_string(),
            is_primary: true,
        };
        let json = serde_json::to_string(&action).expect("serialize");
        let back: RecoveryAction = serde_json::from_str(&json).expect("deserialize");
        assert!(matches!(back.kind, RecoveryKind::CreateMissing));
        assert_eq!(back.label, "Create account");
        assert!(back.is_primary);
    }

    #[test]
    fn recovery_action_is_primary_false_roundtrips() {
        let action = RecoveryAction {
            kind: RecoveryKind::ShowHelp,
            label: "Learn more".to_string(),
            is_primary: false,
        };
        let json = serde_json::to_string(&action).expect("serialize");
        let back: RecoveryAction = serde_json::from_str(&json).expect("deserialize");
        assert!(!back.is_primary);
    }

    // -- RecoveryError --

    #[test]
    fn recovery_error_serializes_with_message_and_recovery_array() {
        let err = RecoveryError {
            message: "Account does not exist".to_string(),
            recovery: NonEmpty::new(
                RecoveryAction {
                    kind: RecoveryKind::CreateMissing,
                    label: "Create account".to_string(),
                    is_primary: true,
                },
                vec![RecoveryAction {
                    kind: RecoveryKind::Discard,
                    label: "Discard".to_string(),
                    is_primary: false,
                }],
            ),
        };
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["message"], "Account does not exist");
        assert_eq!(json["recovery"][0]["kind"], "CREATE_MISSING");
        assert_eq!(json["recovery"][1]["kind"], "DISCARD");
    }

    #[test]
    fn recovery_error_deserializes_from_screaming_snake_keys() {
        let json = serde_json::json!({
            "message": "x",
            "recovery": [{"kind": "SHOW_HELP", "label": "Help", "is_primary": true}],
        });
        let err: RecoveryError = serde_json::from_value(json).unwrap();
        assert_eq!(err.message, "x");
        assert_eq!(err.recovery.first().kind, RecoveryKind::ShowHelp);
    }

    #[test]
    fn recovery_error_show_help_helper_produces_show_help_kind() {
        let err = RecoveryError::show_help("plain message");
        assert_eq!(err.message, "plain message");
        assert_eq!(err.recovery.first().kind, RecoveryKind::ShowHelp);
        assert!(err.recovery.first().is_primary);
    }

    #[test]
    fn recovery_error_discard_helper_produces_discard_kind() {
        let err = RecoveryError::discard("plain message");
        assert_eq!(err.message, "plain message");
        assert_eq!(err.recovery.first().kind, RecoveryKind::Discard);
        assert!(err.recovery.first().is_primary);
    }
}
