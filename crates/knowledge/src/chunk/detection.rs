//! Content type detection for intelligent chunking.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Content type detected for a document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ContentType {
    /// Plain text
    Text,
    
    /// Markdown document
    Markdown,
    
    /// Source code
    Code { language: Language },
    
    /// HTML document
    Html,
    
    /// PDF-converted text
    Pdf,
    
    /// Unknown/unsupported format
    Unknown,
}

/// Programming language detected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    Go,
    C,
    Cpp,
    Java,
    Ruby,
    Php,
    Unknown,
}

impl Language {
    /// Get tree-sitter language for this language.
    pub fn tree_sitter_language(&self) -> Option<tree_sitter::Language> {
        match self {
            Language::Rust => Some(tree_sitter_rust::LANGUAGE.into()),
            Language::TypeScript => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
            Language::JavaScript => Some(tree_sitter_javascript::LANGUAGE.into()),
            Language::Python => Some(tree_sitter_python::LANGUAGE.into()),
            Language::Go => Some(tree_sitter_go::LANGUAGE.into()),
            _ => None,
        }
    }
    
    /// Check if tree-sitter support is available.
    pub fn has_tree_sitter_support(&self) -> bool {
        matches!(
            self,
            Language::Rust
                | Language::TypeScript
                | Language::JavaScript
                | Language::Python
                | Language::Go
        )
    }
}

/// Detect content type from file path and text content.
pub fn detect_content_type(path: Option<&Path>, text: &str) -> ContentType {
    // 1. Extension-based detection
    if let Some(path) = path {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext.to_lowercase().as_str() {
                "rs" => return ContentType::Code { language: Language::Rust },
                "ts" => return ContentType::Code { language: Language::TypeScript },
                "tsx" => return ContentType::Code { language: Language::TypeScript },
                "js" => return ContentType::Code { language: Language::JavaScript },
                "jsx" => return ContentType::Code { language: Language::JavaScript },
                "py" => return ContentType::Code { language: Language::Python },
                "go" => return ContentType::Code { language: Language::Go },
                "c" => return ContentType::Code { language: Language::C },
                "cpp" | "cc" | "cxx" => return ContentType::Code { language: Language::Cpp },
                "java" => return ContentType::Code { language: Language::Java },
                "rb" => return ContentType::Code { language: Language::Ruby },
                "php" => return ContentType::Code { language: Language::Php },
                "md" | "markdown" => return ContentType::Markdown,
                "html" | "htm" => return ContentType::Html,
                "txt" => return ContentType::Text,
                _ => {}
            }
        }
    }

    // 2. Heuristic analysis
    detect_from_content(text)
}

/// Detect content type from text content using heuristics.
fn detect_from_content(text: &str) -> ContentType {
    let trimmed = text.trim();
    
    // Check for HTML
    if trimmed.starts_with("<!DOCTYPE") || trimmed.starts_with("<html") {
        return ContentType::Html;
    }
    
    // Check for common code patterns
    if contains_code_patterns(text) {
        return ContentType::Code {
            language: detect_language_from_content(text),
        };
    }
    
    // Check for markdown patterns
    if contains_markdown_patterns(text) {
        return ContentType::Markdown;
    }
    
    // Default to text
    ContentType::Text
}

/// Check if text contains common code patterns.
fn contains_code_patterns(text: &str) -> bool {
    let code_keywords = [
        "fn ", "func ", "def ", "class ", "import ", "from ", "use ", "package ",
        "const ", "let ", "var ", "function ", "async ", "await ", "return ",
        "if (", "for (", "while (", "switch (", "=> {",
    ];
    
    code_keywords.iter().any(|&keyword| text.contains(keyword))
}

/// Check if text contains markdown patterns.
fn contains_markdown_patterns(text: &str) -> bool {
    let lines: Vec<&str> = text.lines().take(20).collect();
    
    let mut markdown_score = 0;
    
    for line in &lines {
        // Headers
        if line.trim_start().starts_with('#') {
            markdown_score += 2;
        }
        // Lists
        if line.trim_start().starts_with("- ") || line.trim_start().starts_with("* ") {
            markdown_score += 1;
        }
        // Links
        if line.contains("[") && line.contains("](") {
            markdown_score += 2;
        }
        // Code blocks
        if line.trim() == "```" {
            markdown_score += 2;
        }
    }
    
    markdown_score >= 3
}

/// Detect programming language from content.
fn detect_language_from_content(text: &str) -> Language {
    // Rust patterns
    if text.contains("fn ") && (text.contains("impl ") || text.contains("pub ")) {
        return Language::Rust;
    }
    
    // TypeScript/JavaScript patterns
    if text.contains("interface ") || text.contains(": string") || text.contains(": number") {
        return Language::TypeScript;
    }
    
    if text.contains("function ") || text.contains("const ") || text.contains("=> {") {
        return Language::JavaScript;
    }
    
    // Python patterns
    if text.contains("def ") && (text.contains("import ") || text.contains("from ")) {
        return Language::Python;
    }
    
    // Go patterns
    if text.contains("func ") && text.contains("package ") {
        return Language::Go;
    }
    
    Language::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_rust_from_extension() {
        let path = Path::new("test.rs");
        let result = detect_content_type(Some(path), "");
        assert_eq!(result, ContentType::Code { language: Language::Rust });
    }

    #[test]
    fn test_detect_markdown_from_extension() {
        let path = Path::new("README.md");
        let result = detect_content_type(Some(path), "");
        assert_eq!(result, ContentType::Markdown);
    }

    #[test]
    fn test_detect_markdown_from_content() {
        let text = "# Hello\n\nThis is a [link](url)\n\n- Item 1\n- Item 2";
        let result = detect_content_type(None, text);
        assert_eq!(result, ContentType::Markdown);
    }

    #[test]
    fn test_detect_code_from_content() {
        let text = "fn main() {\n    println!(\"Hello\");\n}";
        let result = detect_content_type(None, text);
        assert!(matches!(result, ContentType::Code { .. }));
    }
}
