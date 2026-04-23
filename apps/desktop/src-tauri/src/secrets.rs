// Secret storage (T-049)
// Long-lived credentials (Claude API key) are stored in the OS keychain via
// the `keyring` crate — macOS Keychain / Windows Credential Manager / Linux
// Secret Service. A `CLAUDE_API_KEY` env var overrides the keychain so the
// dev loop doesn't need to touch real OS credentials.
//
// Not stored in SQLCipher: the DB passphrase controls DB access, and is
// distinct from the AI credential — tying them together would make passphrase
// rotation needlessly destructive.

use thiserror::Error;

const SERVICE: &str = "ai.tally.desktop";
const CLAUDE_KEY_USER: &str = "claude-api-key";
const CLAUDE_ENV_VAR: &str = "CLAUDE_API_KEY";

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("keychain error: {0}")]
    Keyring(#[from] keyring::Error),
}

/// Backend abstraction so tests don't touch the real OS keychain.
pub trait SecretStore: Send + Sync {
    fn set(&self, user: &str, value: &str) -> Result<(), SecretError>;
    fn get(&self, user: &str) -> Result<Option<String>, SecretError>;
    fn delete(&self, user: &str) -> Result<(), SecretError>;
}

pub struct KeyringStore {
    service: String,
}

impl KeyringStore {
    pub fn new() -> Self {
        Self { service: SERVICE.to_string() }
    }
}

impl Default for KeyringStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretStore for KeyringStore {
    fn set(&self, user: &str, value: &str) -> Result<(), SecretError> {
        keyring::Entry::new(&self.service, user)?.set_password(value)?;
        Ok(())
    }

    fn get(&self, user: &str) -> Result<Option<String>, SecretError> {
        match keyring::Entry::new(&self.service, user)?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn delete(&self, user: &str) -> Result<(), SecretError> {
        match keyring::Entry::new(&self.service, user)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

/// Resolve the Claude API key: env var wins, then keychain. Returns Ok(None)
/// when neither is set so callers can surface a "please configure" flow.
pub fn load_claude_api_key(store: &dyn SecretStore) -> Result<Option<String>, SecretError> {
    if let Ok(v) = std::env::var(CLAUDE_ENV_VAR) {
        if !v.trim().is_empty() {
            return Ok(Some(v));
        }
    }
    store.get(CLAUDE_KEY_USER)
}

pub fn save_claude_api_key(store: &dyn SecretStore, key: &str) -> Result<(), SecretError> {
    store.set(CLAUDE_KEY_USER, key)
}

pub fn delete_claude_api_key(store: &dyn SecretStore) -> Result<(), SecretError> {
    store.delete(CLAUDE_KEY_USER)
}

pub fn has_claude_api_key(store: &dyn SecretStore) -> Result<bool, SecretError> {
    Ok(load_claude_api_key(store)?.is_some())
}

#[cfg(test)]
pub mod testing {
    //! In-memory store for tests. Public within the crate so command tests can use it too.
    use super::{SecretError, SecretStore};
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct MemoryStore {
        inner: Mutex<HashMap<String, String>>,
    }

    impl MemoryStore {
        pub fn new() -> Self {
            Self::default()
        }
    }

    impl SecretStore for MemoryStore {
        fn set(&self, user: &str, value: &str) -> Result<(), SecretError> {
            self.inner.lock().unwrap().insert(user.to_string(), value.to_string());
            Ok(())
        }

        fn get(&self, user: &str) -> Result<Option<String>, SecretError> {
            Ok(self.inner.lock().unwrap().get(user).cloned())
        }

        fn delete(&self, user: &str) -> Result<(), SecretError> {
            self.inner.lock().unwrap().remove(user);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::MemoryStore;
    use super::*;

    #[test]
    fn load_returns_none_when_nothing_set() {
        // Ensure env isn't set for this test case.
        std::env::remove_var(CLAUDE_ENV_VAR);
        let store = MemoryStore::new();
        assert_eq!(load_claude_api_key(&store).unwrap(), None);
        assert!(!has_claude_api_key(&store).unwrap());
    }

    #[test]
    fn save_then_load_roundtrips_through_store() {
        std::env::remove_var(CLAUDE_ENV_VAR);
        let store = MemoryStore::new();
        save_claude_api_key(&store, "sk-ant-test").unwrap();
        assert_eq!(load_claude_api_key(&store).unwrap(), Some("sk-ant-test".to_string()));
        assert!(has_claude_api_key(&store).unwrap());
    }

    #[test]
    fn delete_removes_the_stored_key() {
        std::env::remove_var(CLAUDE_ENV_VAR);
        let store = MemoryStore::new();
        save_claude_api_key(&store, "sk-ant-test").unwrap();
        delete_claude_api_key(&store).unwrap();
        assert_eq!(load_claude_api_key(&store).unwrap(), None);
    }

    #[test]
    fn delete_is_idempotent_on_missing_entry() {
        std::env::remove_var(CLAUDE_ENV_VAR);
        let store = MemoryStore::new();
        delete_claude_api_key(&store).unwrap(); // should not error
    }
}
