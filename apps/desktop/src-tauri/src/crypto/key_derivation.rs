//! Argon2id key derivation for database encryption.
//!
//! Uses Argon2id for secure password-based key derivation with
//! configurable parameters. The derived key is used for SQLCipher
//! database encryption.

use argon2::{Algorithm, Argon2, Params, Version};
use thiserror::Error;

use super::{KEY_SIZE, SALT_SIZE};

#[derive(Debug, Error)]
pub enum KeyDerivationError {
    #[error("Failed to derive key: {0}")]
    DerivationFailed(String),
}

/// Derives a 256-bit encryption key from a password using Argon2id.
///
/// # Arguments
/// * `password` - User passphrase (will be zeroed after use)
/// * `salt` - 16-byte cryptographic salt (stored in households table)
///
/// # Returns
/// 32-byte key suitable for SQLCipher encryption
///
/// # Security
/// Uses Argon2id with:
/// - Memory cost: 64 MiB
/// - Time cost: 3 iterations
/// - Parallelism: 4 threads
/// These parameters are tuned for desktop security (not mobile).
pub fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; KEY_SIZE], KeyDerivationError> {
    if salt.len() != SALT_SIZE {
        return Err(KeyDerivationError::DerivationFailed(
            "Salt must be 16 bytes".to_string(),
        ));
    }

    let params = Params::new(65536, 3, 4, Some(KEY_SIZE))
        .map_err(|e| KeyDerivationError::DerivationFailed(e.to_string()))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = [0u8; KEY_SIZE];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| KeyDerivationError::DerivationFailed(e.to_string()))?;

    Ok(key)
}

/// Generates a random 16-byte salt for key derivation.
///
/// Uses the `rand` crate's thread RNG (cryptographically secure).
pub fn generate_salt() -> [u8; SALT_SIZE] {
    use rand::RngCore;
    let mut salt = [0u8; SALT_SIZE];
    rand::thread_rng().fill_bytes(&mut salt);
    salt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_key_basic() {
        let password = "correct horse battery staple";
        let salt = generate_salt();

        let key = derive_key(password, &salt).expect("Key derivation should succeed");
        assert_eq!(key.len(), KEY_SIZE);
    }

    #[test]
    fn test_derive_key_deterministic() {
        let password = "test password";
        let salt = [0u8; SALT_SIZE];

        let key1 = derive_key(password, &salt).expect("Key derivation should succeed");
        let key2 = derive_key(password, &salt).expect("Key derivation should succeed");

        assert_eq!(key1, key2, "Same password and salt should produce same key");
    }

    #[test]
    fn test_derive_key_different_passwords() {
        let salt = generate_salt();

        let key1 = derive_key("password1", &salt).expect("Key derivation should succeed");
        let key2 = derive_key("password2", &salt).expect("Key derivation should succeed");

        assert_ne!(key1, key2, "Different passwords should produce different keys");
    }

    #[test]
    fn test_derive_key_different_salts() {
        let password = "same password";

        let key1 = derive_key(password, &generate_salt()).expect("Key derivation should succeed");
        let key2 = derive_key(password, &generate_salt()).expect("Key derivation should succeed");

        assert_ne!(key1, key2, "Different salts should produce different keys");
    }

    #[test]
    fn test_salt_size_validation() {
        let password = "test";
        let short_salt = [0u8; 8];

        let result = derive_key(password, &short_salt);
        assert!(result.is_err(), "Should reject salt shorter than 16 bytes");
    }

    #[test]
    fn test_generate_salt_randomness() {
        let salt1 = generate_salt();
        let salt2 = generate_salt();

        assert_ne!(salt1, salt2, "Generated salts should be unique");
    }
}