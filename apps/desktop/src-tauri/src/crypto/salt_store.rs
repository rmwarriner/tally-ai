use std::path::{Path, PathBuf};
use std::io;
use super::SALT_SIZE;

fn salt_path(db_path: &Path) -> PathBuf {
    let mut p = db_path.to_path_buf();
    let mut name = p
        .file_name()
        .unwrap_or_default()
        .to_os_string();
    name.push(".salt");
    p.set_file_name(name);
    p
}

/// Persists a salt alongside the database file as `{db_path}.salt`.
pub fn save_salt(db_path: &Path, salt: &[u8; SALT_SIZE]) -> Result<(), io::Error> {
    std::fs::write(salt_path(db_path), salt)
}

/// Loads the salt for a database. Returns `None` if the salt file does not exist
/// or is the wrong length (indicating an uninitialized or corrupt key file).
pub fn load_salt(db_path: &Path) -> Option<[u8; SALT_SIZE]> {
    let bytes = std::fs::read(salt_path(db_path)).ok()?;
    if bytes.len() != SALT_SIZE {
        return None;
    }
    let mut arr = [0u8; SALT_SIZE];
    arr.copy_from_slice(&bytes);
    Some(arr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_save_and_load_salt_roundtrip() {
        let dir = tempdir().expect("Should create temp dir");
        let db_path = dir.path().join("tally.db");
        let salt: [u8; SALT_SIZE] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];

        save_salt(&db_path, &salt).expect("Should save salt");
        let loaded = load_salt(&db_path).expect("Should load salt");

        assert_eq!(salt, loaded, "Salt should round-trip exactly");
    }

    #[test]
    fn test_load_salt_returns_none_when_missing() {
        let dir = tempdir().expect("Should create temp dir");
        let db_path = dir.path().join("nonexistent.db");

        assert!(
            load_salt(&db_path).is_none(),
            "Should return None when salt file does not exist"
        );
    }

    #[test]
    fn test_load_salt_returns_none_for_wrong_length() {
        let dir = tempdir().expect("Should create temp dir");
        let db_path = dir.path().join("bad.db");
        let salt_path = dir.path().join("bad.db.salt");

        std::fs::write(&salt_path, b"tooshort").expect("Should write bad salt file");

        assert!(
            load_salt(&db_path).is_none(),
            "Should return None when salt file has wrong length"
        );
    }

    #[test]
    fn test_salt_file_path_is_alongside_db() {
        let db_path = Path::new("/data/tally.db");
        let sp = salt_path(db_path);
        assert_eq!(sp, Path::new("/data/tally.db.salt"));
    }

    #[test]
    fn test_save_overwrites_existing_salt() {
        let dir = tempdir().expect("Should create temp dir");
        let db_path = dir.path().join("tally.db");

        let salt1 = [0u8; SALT_SIZE];
        let salt2 = [255u8; SALT_SIZE];

        save_salt(&db_path, &salt1).expect("Should save first salt");
        save_salt(&db_path, &salt2).expect("Should overwrite with second salt");

        let loaded = load_salt(&db_path).expect("Should load salt");
        assert_eq!(loaded, salt2, "Should load the most recently saved salt");
    }
}
