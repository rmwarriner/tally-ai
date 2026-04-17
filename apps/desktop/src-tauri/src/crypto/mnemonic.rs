//! BIP-39 mnemonic generation for recovery codes.
//!
//! Generates 12-word mnemonic phrases for household key recovery.
//! These mnemonics can be used to derive the same encryption key
//! as the original passphrase when combined with the stored salt.

use bip39::{Language, Mnemonic};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MnemonicError {
    #[error("Failed to generate mnemonic: {0}")]
    GenerationFailed(String),
    #[error("Invalid mnemonic: {0}")]
    InvalidMnemonic(String),
}

/// Generates a new 12-word BIP-39 mnemonic phrase.
///
/// # Returns
/// A string containing 12 space-separated English words
///
/// # Security
/// Uses cryptographically secure random number generation.
/// The mnemonic should be displayed to the user once during setup
/// and never stored or logged.
pub fn generate_mnemonic() -> Result<String, MnemonicError> {
    let mnemonic = Mnemonic::generate(12)
        .map_err(|e| MnemonicError::GenerationFailed(e.to_string()))?;
    Ok(mnemonic.to_string())
}

/// Converts a mnemonic phrase back to entropy bytes.
///
/// # Arguments
/// * `phrase` - 12-word space-separated mnemonic phrase
///
/// # Returns
/// Raw entropy bytes (16 bytes for 12-word mnemonic)
///
/// # Errors
/// Returns error if phrase is invalid or contains unknown words
#[allow(dead_code)]
pub fn mnemonic_to_entropy(phrase: &str) -> Result<Vec<u8>, MnemonicError> {
    let mnemonic = Mnemonic::parse_in(Language::English, phrase)
        .map_err(|e| MnemonicError::InvalidMnemonic(e.to_string()))?;
    Ok(mnemonic.to_entropy())
}

/// Validates that a mnemonic phrase is well-formed.
///
/// # Arguments
/// * `phrase` - Space-separated mnemonic words
///
/// # Returns
/// true if the phrase is a valid 12-word BIP-39 mnemonic
#[allow(dead_code)]
pub fn validate_mnemonic(phrase: &str) -> bool {
    Mnemonic::parse_in(Language::English, phrase).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_mnemonic() {
        let mnemonic = generate_mnemonic().expect("Should generate mnemonic");
        let words: Vec<&str> = mnemonic.split_whitespace().collect();
        assert_eq!(words.len(), 12, "Should generate 12 words");
    }

    #[test]
    fn test_generate_mnemonic_uniqueness() {
        let m1 = generate_mnemonic().expect("Should generate mnemonic");
        let m2 = generate_mnemonic().expect("Should generate mnemonic");
        assert_ne!(m1, m2, "Generated mnemonics should be unique");
    }

    #[test]
    fn test_validate_mnemonic_valid() {
        // Use a known valid mnemonic
        let valid_phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        assert!(validate_mnemonic(valid_phrase), "Should validate known mnemonic");
    }

    #[test]
    fn test_validate_mnemonic_invalid() {
        assert!(!validate_mnemonic("invalid words here"), "Should reject invalid mnemonic");
        assert!(!validate_mnemonic(""), "Should reject empty string");
        assert!(!validate_mnemonic("abandon abandon abandon"), "Should reject short mnemonic");
    }

    #[test]
    fn test_mnemonic_to_entropy() {
        let mnemonic = generate_mnemonic().expect("Should generate mnemonic");
        let entropy = mnemonic_to_entropy(&mnemonic).expect("Should extract entropy");
        assert_eq!(entropy.len(), 16, "12-word mnemonic has 16 bytes of entropy");
    }

    #[test]
    fn test_mnemonic_roundtrip() {
        let original = generate_mnemonic().expect("Should generate mnemonic");
        let entropy = mnemonic_to_entropy(&original).expect("Should extract entropy");

        // Create mnemonic from entropy
        let mnemonic = Mnemonic::from_entropy(&entropy)
            .expect("Should create mnemonic from entropy");
        let recovered = mnemonic.to_string();

        assert_eq!(original, recovered, "Mnemonic should round-trip correctly");
    }
}