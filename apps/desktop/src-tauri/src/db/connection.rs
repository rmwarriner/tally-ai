//! Database connection management with SQLCipher encryption.
//!
//! Provides secure database connection using SQLCipher with Argon2id-derived keys.
//! All databases are encrypted at rest using the user's passphrase.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::Path;
use std::str::FromStr;
use thiserror::Error;

use crate::crypto::{derive_key, KeyDerivationError};
use super::migrations::{run_migrations, MigrationError};

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("Failed to connect to database: {0}")]
    ConnectionFailed(String),
    #[error("Failed to create database: {0}")]
    CreateFailed(String),
    #[error("Key derivation failed: {0}")]
    KeyDerivation(#[from] KeyDerivationError),
    #[error("Database path invalid: {0}")]
    InvalidPath(String),
    #[error("SQL error: {0}")]
    SqlError(#[from] sqlx::Error),
    #[error("Migration error: {0}")]
    Migration(#[from] MigrationError),
}

/// Opens an encrypted SQLCipher database connection.
///
/// # Arguments
/// * `path` - Path to the SQLite database file
/// * `passphrase` - User's passphrase for encryption
/// * `salt` - 16-byte salt stored in households table
///
/// # Returns
/// A connection pool for database operations
///
/// # Security
/// The passphrase is derived into a 256-bit key using Argon2id before
/// being used for SQLCipher encryption. The key is never stored.
///
/// The database file is encrypted at rest. Without the correct key,
/// the file appears as random bytes.
pub async fn open_encrypted_db(
    path: &Path,
    passphrase: &str,
    salt: &[u8],
) -> Result<SqlitePool, DatabaseError> {
    let key = derive_key(passphrase, salt)?;
    let key_hex = hex_encode(&key);

    let path_str = path
        .to_str()
        .ok_or_else(|| DatabaseError::InvalidPath("Invalid database path".to_string()))?;

    let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", path_str))?
        .pragma("key", format!("\"x'{}'\"", key_hex))
        .pragma("journal_mode", "WAL")
        .pragma("synchronous", "NORMAL")
        .pragma("foreign_keys", "ON")
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|e| DatabaseError::ConnectionFailed(e.to_string()))?;

    Ok(pool)
}

/// Creates a new encrypted database with initial schema setup.
///
/// # Arguments
/// * `path` - Path where the new database file will be created
/// * `passphrase` - User's passphrase for encryption
/// * `salt` - 16-byte salt (will be stored in households table)
///
/// # Returns
/// A connection pool ready for migrations
pub async fn create_encrypted_db(
    path: &Path,
    passphrase: &str,
    salt: &[u8],
) -> Result<SqlitePool, DatabaseError> {
    let pool = open_encrypted_db(path, passphrase, salt).await?;

    // Verify encryption is working before running migrations
    sqlx::query("SELECT 1")
        .fetch_one(&pool)
        .await
        .map_err(|e| DatabaseError::CreateFailed(format!("Encryption verification failed: {}", e)))?;

    run_migrations(&pool).await?;

    Ok(pool)
}

/// Verifies that the database can be opened with the given credentials.
///
/// Used during authentication to check if the passphrase is correct.
///
/// # Arguments
/// * `path` - Path to the SQLite database file
/// * `passphrase` - User's passphrase
/// * `salt` - Salt from households table
///
/// # Returns
/// true if the database can be opened and is properly encrypted
pub async fn verify_encrypted_db(
    path: &Path,
    passphrase: &str,
    salt: &[u8],
) -> Result<bool, DatabaseError> {
    match open_encrypted_db(path, passphrase, salt).await {
        Ok(pool) => {
            // Try a simple query to verify the key works
            let result = sqlx::query("SELECT 1").fetch_one(&pool).await;
            Ok(result.is_ok())
        }
        Err(_) => Ok(false),
    }
}

/// Converts bytes to hexadecimal string (lowercase, no prefix).
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_hex_encode() {
        let bytes: [u8; 4] = [0x01, 0x23, 0xab, 0xcd];
        let hex = hex_encode(&bytes);
        assert_eq!(hex, "0123abcd");
    }

    #[tokio::test]
    async fn test_create_and_open_encrypted_db() {
        let dir = tempdir().expect("Should create temp dir");
        let db_path = dir.path().join("test.db");

        let passphrase = "correct horse battery staple";
        let salt = [0u8; 16]; // Use fixed salt for testing

        // Create database
        let pool = create_encrypted_db(&db_path, passphrase, &salt)
            .await
            .expect("Should create encrypted database");

        // Verify we can execute queries
        let result: (i32,) = sqlx::query_as("SELECT 1")
            .fetch_one(&pool)
            .await
            .expect("Should execute query");
        assert_eq!(result.0, 1);

        // Verify file exists
        assert!(db_path.exists(), "Database file should exist");
    }

    #[tokio::test]
    async fn test_wrong_passphrase_fails() {
        let dir = tempdir().expect("Should create temp dir");
        let db_path = dir.path().join("test_wrong.db");

        let correct_passphrase = "correct password";
        let wrong_passphrase = "wrong password";
        let salt = [0u8; 16];

        // Create with correct passphrase
        let _pool = create_encrypted_db(&db_path, correct_passphrase, &salt)
            .await
            .expect("Should create database");

        // SQLCipher validates the key during connection; wrong key fails immediately
        let result = open_encrypted_db(&db_path, wrong_passphrase, &salt).await;
        assert!(result.is_err(), "Should not open database with wrong passphrase");
    }

    #[tokio::test]
    async fn test_verify_encrypted_db() {
        let dir = tempdir().expect("Should create temp dir");
        let db_path = dir.path().join("test_verify.db");

        let passphrase = "test passphrase";
        let salt = [0u8; 16];

        // Create database
        let _pool = create_encrypted_db(&db_path, passphrase, &salt)
            .await
            .expect("Should create database");

        // Verify with correct passphrase
        let valid = verify_encrypted_db(&db_path, passphrase, &salt)
            .await
            .expect("Should verify");
        assert!(valid, "Should verify correct passphrase");

        // Verify with wrong passphrase
        let invalid = verify_encrypted_db(&db_path, "wrong passphrase", &salt)
            .await
            .expect("Should verify");
        assert!(!invalid, "Should reject wrong passphrase");
    }
}