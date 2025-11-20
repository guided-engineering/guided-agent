//! Metadata module for knowledge chunks
//!
//! Provides file type classification, language detection, tag derivation,
//! and content hashing for knowledge chunks.

mod detect;
mod types;

pub use detect::{detect_file_type, detect_language, derive_tags};
pub use types::{ContentType, FileType, Language, Metadata};

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::path::Path;

/// Generate SHA-256 content hash for deduplication
pub fn generate_content_hash(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)
}

/// Extract metadata from file path and content
pub fn extract_metadata(path: &Path, content: &str) -> Metadata {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let file_type = detect_file_type(path);
    let language = detect_language(path, content, &file_type);
    let tags = derive_tags(path);

    let file_metadata = std::fs::metadata(path).ok();
    let file_size_bytes = file_metadata.as_ref().map(|m| m.len()).unwrap_or(0);
    let file_modified_at = file_metadata
        .as_ref()
        .and_then(|m| m.modified().ok())
        .and_then(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| DateTime::from_timestamp(d.as_secs() as i64, 0))
        })
        .flatten()
        .unwrap_or_else(Utc::now);

    let line_count = content.lines().count();
    let content_hash = generate_content_hash(content);

    Metadata {
        source_path: path.to_string_lossy().to_string(),
        file_name,
        file_type,
        language,
        file_size_bytes,
        file_line_count: line_count,
        file_modified_at,
        content_hash,
        tags,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_generate_content_hash() {
        let text = "Hello, world!";
        let hash = generate_content_hash(text);
        assert_eq!(hash.len(), 64); // SHA-256 produces 64 hex chars

        // Same content produces same hash
        let hash2 = generate_content_hash(text);
        assert_eq!(hash, hash2);

        // Different content produces different hash
        let hash3 = generate_content_hash("Different text");
        assert_ne!(hash, hash3);
    }

    #[test]
    fn test_extract_metadata() {
        let path = PathBuf::from("docs/api/test.rs");
        let content = "fn main() {\n    println!(\"Hello\");\n}";

        let metadata = extract_metadata(&path, content);

        assert_eq!(metadata.file_name, "test.rs");
        assert!(matches!(metadata.file_type, FileType::Code(_)));
        assert!(matches!(metadata.language, Some(Language::Rust)));
        assert_eq!(metadata.file_line_count, 3);
        assert!(metadata.content_hash.len() == 64);
        assert!(metadata.tags.contains(&"docs".to_string()));
        assert!(metadata.tags.contains(&"api".to_string()));
    }
}
