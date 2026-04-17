//! Cryptographic utilities for Tally.ai
//!
//! Provides Argon2id key derivation and BIP-39 mnemonic generation
//! for SQLCipher database encryption.

mod key_derivation;
mod mnemonic;

pub use key_derivation::{derive_key, generate_salt, KeyDerivationError};
pub use mnemonic::{generate_mnemonic, MnemonicError};

/// Size of the derived encryption key in bytes (256 bits)
pub const KEY_SIZE: usize = 32;

/// Size of the salt in bytes
pub const SALT_SIZE: usize = 16;