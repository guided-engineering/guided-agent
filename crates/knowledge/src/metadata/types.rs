//! Metadata types for knowledge chunks

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Content type classification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentType {
    Text,
    Markdown,
    Code,
    Html,
    Pdf,
    Json,
    Yaml,
    Xml,
    Unknown,
}

/// File type with programming language for code files
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileType {
    Markdown,
    Html,
    Pdf,
    Code(String), // Programming language: "rust", "typescript", etc.
    Text,
    Json,
    Yaml,
    Xml,
    Unknown,
}

impl FileType {
    /// Get the programming language if this is a code file
    pub fn language(&self) -> Option<&str> {
        match self {
            FileType::Code(lang) => Some(lang),
            _ => None,
        }
    }

    /// Check if this is a code file
    pub fn is_code(&self) -> bool {
        matches!(self, FileType::Code(_))
    }

    /// Convert to string representation
    pub fn as_str(&self) -> &str {
        match self {
            FileType::Markdown => "markdown",
            FileType::Html => "html",
            FileType::Pdf => "pdf",
            FileType::Code(_) => "code",
            FileType::Text => "text",
            FileType::Json => "json",
            FileType::Yaml => "yaml",
            FileType::Xml => "xml",
            FileType::Unknown => "unknown",
        }
    }
}

/// Language classification (programming or natural)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    // Natural languages
    #[serde(rename = "portuguese")]
    Portuguese,
    #[serde(rename = "english")]
    English,
    #[serde(rename = "spanish")]
    Spanish,
    #[serde(rename = "french")]
    French,

    // Programming languages
    #[serde(rename = "rust")]
    Rust,
    #[serde(rename = "typescript")]
    TypeScript,
    #[serde(rename = "javascript")]
    JavaScript,
    #[serde(rename = "python")]
    Python,
    #[serde(rename = "go")]
    Go,
    #[serde(rename = "java")]
    Java,
    #[serde(rename = "cpp")]
    Cpp,
    #[serde(rename = "c")]
    C,
    #[serde(rename = "csharp")]
    CSharp,
    #[serde(rename = "ruby")]
    Ruby,
    #[serde(rename = "php")]
    Php,
    #[serde(rename = "swift")]
    Swift,
    #[serde(rename = "kotlin")]
    Kotlin,

    #[serde(rename = "unknown")]
    Unknown,
}

impl Language {
    /// Check if this is a programming language
    pub fn is_programming(&self) -> bool {
        !matches!(
            self,
            Language::Portuguese
                | Language::English
                | Language::Spanish
                | Language::French
                | Language::Unknown
        )
    }

    /// Check if this is a natural language
    pub fn is_natural(&self) -> bool {
        matches!(
            self,
            Language::Portuguese | Language::English | Language::Spanish | Language::French
        )
    }

    /// Convert to string representation
    pub fn as_str(&self) -> &str {
        match self {
            Language::Portuguese => "portuguese",
            Language::English => "english",
            Language::Spanish => "spanish",
            Language::French => "french",
            Language::Rust => "rust",
            Language::TypeScript => "typescript",
            Language::JavaScript => "javascript",
            Language::Python => "python",
            Language::Go => "go",
            Language::Java => "java",
            Language::Cpp => "cpp",
            Language::C => "c",
            Language::CSharp => "csharp",
            Language::Ruby => "ruby",
            Language::Php => "php",
            Language::Swift => "swift",
            Language::Kotlin => "kotlin",
            Language::Unknown => "unknown",
        }
    }
}

/// Complete metadata for a knowledge chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    /// Full source path
    pub source_path: String,

    /// File name only
    pub file_name: String,

    /// File type classification
    pub file_type: FileType,

    /// Language (programming or natural)
    pub language: Option<Language>,

    /// File size in bytes
    pub file_size_bytes: u64,

    /// Number of lines in file
    pub file_line_count: usize,

    /// File modification timestamp
    pub file_modified_at: DateTime<Utc>,

    /// Content hash (SHA-256)
    pub content_hash: String,

    /// Tags derived from path or content
    pub tags: Vec<String>,

    /// Chunk creation timestamp
    pub created_at: DateTime<Utc>,

    /// Chunk update timestamp
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_type_is_code() {
        assert!(FileType::Code("rust".to_string()).is_code());
        assert!(!FileType::Markdown.is_code());
        assert!(!FileType::Text.is_code());
    }

    #[test]
    fn test_file_type_language() {
        let file_type = FileType::Code("rust".to_string());
        assert_eq!(file_type.language(), Some("rust"));

        let file_type = FileType::Markdown;
        assert_eq!(file_type.language(), None);
    }

    #[test]
    fn test_language_is_programming() {
        assert!(Language::Rust.is_programming());
        assert!(Language::Python.is_programming());
        assert!(!Language::English.is_programming());
        assert!(!Language::Portuguese.is_programming());
    }

    #[test]
    fn test_language_is_natural() {
        assert!(Language::English.is_natural());
        assert!(Language::Portuguese.is_natural());
        assert!(!Language::Rust.is_natural());
        assert!(!Language::Python.is_natural());
    }

    #[test]
    fn test_file_type_as_str() {
        assert_eq!(FileType::Markdown.as_str(), "markdown");
        assert_eq!(FileType::Code("rust".to_string()).as_str(), "code");
        assert_eq!(FileType::Json.as_str(), "json");
    }

    #[test]
    fn test_language_as_str() {
        assert_eq!(Language::Rust.as_str(), "rust");
        assert_eq!(Language::Portuguese.as_str(), "portuguese");
        assert_eq!(Language::TypeScript.as_str(), "typescript");
    }
}
