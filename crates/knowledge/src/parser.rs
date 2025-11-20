//! Source file parsing and text extraction.

use guided_core::{AppError, AppResult};
use std::fs;
use std::path::Path;

/// Content type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    Markdown,
    Html,
    Code,
    PlainText,
    Unknown,
}

impl ContentType {
    /// Detect content type from file extension.
    pub fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|e| e.to_str()) {
            Some("md") | Some("markdown") => Self::Markdown,
            Some("html") | Some("htm") => Self::Html,
            Some("rs") | Some("py") | Some("js") | Some("ts") | Some("go") | Some("c")
            | Some("cpp") | Some("java") | Some("sh") | Some("yaml") | Some("yml")
            | Some("json") | Some("toml") => Self::Code,
            Some("txt") => Self::PlainText,
            _ => Self::Unknown,
        }
    }

    /// Get string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::Html => "html",
            Self::Code => "code",
            Self::PlainText => "text",
            Self::Unknown => "unknown",
        }
    }
}

/// Parse a source file and extract clean text.
pub fn parse_file(path: &Path) -> AppResult<String> {
    let content_type = ContentType::from_path(path);

    let raw = fs::read_to_string(path)
        .map_err(|e| AppError::Knowledge(format!("Failed to read {:?}: {}", path, e)))?;

    let cleaned = match content_type {
        ContentType::Markdown => clean_markdown(&raw),
        ContentType::Html => clean_html(&raw),
        ContentType::Code => clean_code(&raw),
        ContentType::PlainText => raw,
        ContentType::Unknown => {
            // Try to read as text, skip if binary
            if is_likely_text(&raw) {
                raw
            } else {
                tracing::warn!("Skipping likely binary file: {:?}", path);
                return Err(AppError::Knowledge("Binary file not supported".to_string()));
            }
        }
    };

    Ok(cleaned)
}

/// Clean markdown by removing excess formatting.
fn clean_markdown(text: &str) -> String {
    let mut result = String::with_capacity(text.len());

    for line in text.lines() {
        // Remove markdown headers
        let trimmed = line.trim_start_matches('#').trim();

        // Skip horizontal rules and code fences
        if trimmed.starts_with("---") || trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            continue;
        }

        // Keep content
        if !trimmed.is_empty() {
            result.push_str(trimmed);
            result.push('\n');
        }
    }

    result.trim().to_string()
}

/// Clean HTML by stripping tags (simple approach).
fn clean_html(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_tag = false;
    let mut in_script = false;
    let mut in_style = false;

    let lower = text.to_lowercase();

    for (i, ch) in text.chars().enumerate() {
        if ch == '<' {
            in_tag = true;

            // Check for script/style tags
            if lower[i..].starts_with("<script") {
                in_script = true;
            } else if lower[i..].starts_with("</script") {
                in_script = false;
            } else if lower[i..].starts_with("<style") {
                in_style = true;
            } else if lower[i..].starts_with("</style") {
                in_style = false;
            }
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag && !in_script && !in_style {
            result.push(ch);
        }
    }

    // Collapse whitespace
    result
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

/// Clean code by removing excess whitespace and comments (simple approach).
fn clean_code(text: &str) -> String {
    let mut result = String::with_capacity(text.len());

    for line in text.lines() {
        let trimmed = line.trim();

        // Skip single-line comments (basic detection)
        if trimmed.starts_with("//") || trimmed.starts_with('#') {
            continue;
        }

        if !trimmed.is_empty() {
            result.push_str(trimmed);
            result.push('\n');
        }
    }

    result.trim().to_string()
}

/// Check if text is likely UTF-8 text (not binary).
fn is_likely_text(data: &str) -> bool {
    // Simple heuristic: check for null bytes
    !data.contains('\0')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_type_detection() {
        assert_eq!(
            ContentType::from_path(Path::new("file.md")),
            ContentType::Markdown
        );
        assert_eq!(
            ContentType::from_path(Path::new("file.rs")),
            ContentType::Code
        );
        assert_eq!(
            ContentType::from_path(Path::new("file.txt")),
            ContentType::PlainText
        );
    }

    #[test]
    fn test_clean_markdown() {
        let input = "# Header\n\nSome text\n\n```rust\ncode\n```\n\nMore text";
        let output = clean_markdown(input);
        assert!(output.contains("Header"));
        assert!(output.contains("Some text"));
        assert!(output.contains("More text"));
        assert!(!output.contains("```"));
    }

    #[test]
    fn test_clean_html() {
        let input = "<html><body><p>Hello <b>world</b></p></body></html>";
        let output = clean_html(input);
        assert_eq!(output, "Hello world");
    }

    #[test]
    fn test_clean_code() {
        let input = "// Comment\nfn main() {\n    println!(\"hello\");\n}";
        let output = clean_code(input);
        assert!(!output.contains("// Comment"));
        assert!(output.contains("fn main()"));
    }
}
