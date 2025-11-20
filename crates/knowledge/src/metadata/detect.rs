//! Detection logic for file types, languages, and tags

use super::types::{FileType, Language};
use std::path::Path;

/// Detect file type from path extension
pub fn detect_file_type(path: &Path) -> FileType {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match extension.as_str() {
        // Markup/docs
        "md" | "markdown" => FileType::Markdown,
        "html" | "htm" => FileType::Html,
        "pdf" => FileType::Pdf,

        // Data formats
        "json" => FileType::Json,
        "yaml" | "yml" => FileType::Yaml,
        "xml" => FileType::Xml,
        "txt" => FileType::Text,

        // Programming languages
        "rs" => FileType::Code("rust".to_string()),
        "ts" => FileType::Code("typescript".to_string()),
        "tsx" => FileType::Code("typescript".to_string()),
        "js" => FileType::Code("javascript".to_string()),
        "jsx" => FileType::Code("javascript".to_string()),
        "py" => FileType::Code("python".to_string()),
        "go" => FileType::Code("go".to_string()),
        "java" => FileType::Code("java".to_string()),
        "cpp" | "cc" | "cxx" => FileType::Code("cpp".to_string()),
        "c" => FileType::Code("c".to_string()),
        "cs" => FileType::Code("csharp".to_string()),
        "rb" => FileType::Code("ruby".to_string()),
        "php" => FileType::Code("php".to_string()),
        "swift" => FileType::Code("swift".to_string()),
        "kt" | "kts" => FileType::Code("kotlin".to_string()),
        "sh" | "bash" => FileType::Code("shell".to_string()),
        "sql" => FileType::Code("sql".to_string()),

        _ => FileType::Unknown,
    }
}

/// Detect language from path and content
pub fn detect_language(path: &Path, content: &str, file_type: &FileType) -> Option<Language> {
    // For code files, derive language from file type
    if let FileType::Code(lang) = file_type {
        return match lang.as_str() {
            "rust" => Some(Language::Rust),
            "typescript" => Some(Language::TypeScript),
            "javascript" => Some(Language::JavaScript),
            "python" => Some(Language::Python),
            "go" => Some(Language::Go),
            "java" => Some(Language::Java),
            "cpp" => Some(Language::Cpp),
            "c" => Some(Language::C),
            "csharp" => Some(Language::CSharp),
            "ruby" => Some(Language::Ruby),
            "php" => Some(Language::Php),
            "swift" => Some(Language::Swift),
            "kotlin" => Some(Language::Kotlin),
            _ => None,
        };
    }

    // For text files, detect natural language using heuristics
    detect_natural_language(content)
}

/// Detect natural language using simple heuristics
fn detect_natural_language(content: &str) -> Option<Language> {
    // Sample first 500 chars for detection
    let sample = content.chars().take(500).collect::<String>().to_lowercase();

    // Portuguese indicators
    let pt_indicators = [
        "não", "você", "também", "está", "será", "é", "são", "português", "função", "código",
    ];
    let pt_score = pt_indicators.iter().filter(|&w| sample.contains(w)).count();

    // Spanish indicators
    let es_indicators = [
        "está", "usted", "también", "será", "español", "función", "código", "ñ",
    ];
    let es_score = es_indicators.iter().filter(|&w| sample.contains(w)).count();

    // English indicators (default for most technical content)
    let en_indicators = [
        "the", "is", "are", "was", "were", "function", "class", "code", "this", "that",
    ];
    let en_score = en_indicators.iter().filter(|&w| sample.contains(w)).count();

    // Return language with highest score, or English as default
    if pt_score > en_score && pt_score > es_score {
        Some(Language::Portuguese)
    } else if es_score > en_score && es_score > pt_score {
        Some(Language::Spanish)
    } else if en_score > 0 {
        Some(Language::English)
    } else {
        // Default to English for technical content
        Some(Language::English)
    }
}

/// Derive tags from file path
pub fn derive_tags(path: &Path) -> Vec<String> {
    let mut tags = Vec::new();

    // Extract directory names as tags
    for component in path.components() {
        if let std::path::Component::Normal(dir) = component {
            if let Some(dir_str) = dir.to_str() {
                // Skip common root directories
                if matches!(dir_str, "." | ".." | "/" | "src" | "lib" | "target" | "node_modules") {
                    continue;
                }

                // Add directory as tag
                tags.push(dir_str.to_lowercase());
            }
        }
    }

    // Add special tags based on path patterns
    let path_str = path.to_string_lossy().to_lowercase();

    if path_str.contains("test") || path_str.contains("spec") {
        tags.push("test".to_string());
    }
    if path_str.contains("doc") {
        tags.push("docs".to_string());
    }
    if path_str.contains("api") {
        tags.push("api".to_string());
    }
    if path_str.contains("util") || path_str.contains("helper") {
        tags.push("utils".to_string());
    }
    if path_str.contains("config") {
        tags.push("config".to_string());
    }

    // Deduplicate
    tags.sort();
    tags.dedup();

    tags
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_detect_file_type_code() {
        assert!(matches!(
            detect_file_type(&PathBuf::from("test.rs")),
            FileType::Code(_)
        ));
        assert!(matches!(
            detect_file_type(&PathBuf::from("test.ts")),
            FileType::Code(_)
        ));
        assert!(matches!(
            detect_file_type(&PathBuf::from("test.py")),
            FileType::Code(_)
        ));
    }

    #[test]
    fn test_detect_file_type_markup() {
        assert_eq!(
            detect_file_type(&PathBuf::from("test.md")),
            FileType::Markdown
        );
        assert_eq!(
            detect_file_type(&PathBuf::from("test.html")),
            FileType::Html
        );
    }

    #[test]
    fn test_detect_file_type_data() {
        assert_eq!(
            detect_file_type(&PathBuf::from("test.json")),
            FileType::Json
        );
        assert_eq!(
            detect_file_type(&PathBuf::from("test.yaml")),
            FileType::Yaml
        );
    }

    #[test]
    fn test_detect_language_code() {
        let path = PathBuf::from("test.rs");
        let file_type = FileType::Code("rust".to_string());
        let content = "fn main() {}";

        let lang = detect_language(&path, content, &file_type);
        assert_eq!(lang, Some(Language::Rust));
    }

    #[test]
    fn test_detect_natural_language_portuguese() {
        let content = "Este é um texto em português. Você pode ver que não está em inglês.";
        let lang = detect_natural_language(content);
        assert_eq!(lang, Some(Language::Portuguese));
    }

    #[test]
    fn test_detect_natural_language_english() {
        let content = "This is a text in English. You can see that it is not in Portuguese.";
        let lang = detect_natural_language(content);
        assert_eq!(lang, Some(Language::English));
    }

    #[test]
    fn test_derive_tags() {
        let path = PathBuf::from("src/api/docs/test.md");
        let tags = derive_tags(&path);

        assert!(tags.contains(&"api".to_string()));
        assert!(tags.contains(&"docs".to_string()));
        assert!(tags.contains(&"test".to_string()));
    }

    #[test]
    fn test_derive_tags_utils() {
        let path = PathBuf::from("utils/helper.ts");
        let tags = derive_tags(&path);

        assert!(tags.contains(&"utils".to_string()));
    }

    #[test]
    fn test_derive_tags_dedup() {
        let path = PathBuf::from("docs/api/docs/test.md");
        let tags = derive_tags(&path);

        // Should only have one "docs" tag
        assert_eq!(tags.iter().filter(|t| *t == "docs").count(), 1);
    }
}
