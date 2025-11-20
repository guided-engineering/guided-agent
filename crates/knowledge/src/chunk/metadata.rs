//! Metadata enrichment utilities.

use sha2::{Digest, Sha256};

/// Calculate SHA-256 hash of text.
pub fn calculate_hash(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_hash() {
        let text = "Hello, world!";
        let hash = calculate_hash(text);
        assert_eq!(hash.len(), 64); // SHA-256 produces 64 hex chars
        
        // Same text should produce same hash
        let hash2 = calculate_hash(text);
        assert_eq!(hash, hash2);
        
        // Different text should produce different hash
        let hash3 = calculate_hash("Different text");
        assert_ne!(hash, hash3);
    }
}
