use ulid::Ulid;

pub fn new_ulid() -> String {
    Ulid::new().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_ulid_is_26_chars() {
        let id = new_ulid();
        assert_eq!(id.len(), 26, "ULID should be 26 characters");
    }

    #[test]
    fn test_new_ulid_is_unique() {
        let id1 = new_ulid();
        let id2 = new_ulid();
        assert_ne!(id1, id2, "Two ULIDs should be different");
    }

    #[test]
    fn test_new_ulid_timestamp_ordering() {
        // Ulid::new() uses random bits within the same millisecond so back-to-back
        // calls are not guaranteed ordered. Verify ordering across distinct timestamps
        // by constructing ULIDs from known millisecond values.
        use std::time::{Duration, UNIX_EPOCH};
        let t1 = UNIX_EPOCH + Duration::from_millis(1_000_000);
        let t2 = UNIX_EPOCH + Duration::from_millis(2_000_000);
        let id1 = Ulid::from_datetime(t1).to_string();
        let id2 = Ulid::from_datetime(t2).to_string();
        assert!(id1 < id2, "ULID with earlier timestamp must sort before later one");
    }

    #[test]
    fn test_new_ulid_is_uppercase() {
        let id = new_ulid();
        assert_eq!(
            id, id.to_uppercase(),
            "ULID should use uppercase Crockford base32"
        );
    }
}
