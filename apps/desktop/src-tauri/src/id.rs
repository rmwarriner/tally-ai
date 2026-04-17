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
    fn test_new_ulid_monotonically_ordered() {
        let id1 = new_ulid();
        let id2 = new_ulid();
        assert!(id1 <= id2, "ULIDs should be monotonically ordered");
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
