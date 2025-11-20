//! Code splitter using tree-sitter for semantic code chunking.

use super::ChunkSplitter;
use crate::chunk::{detection::{ContentType, Language}, Chunk, ChunkConfig};
use guided_core::{AppError, AppResult};
use tree_sitter::Parser;

pub struct CodeSplitter {
    pub language: Language,
}

impl CodeSplitter {
    pub fn new(language: Language) -> Self {
        Self { language }
    }
}

impl ChunkSplitter for CodeSplitter {
    fn split(&self, source_id: &str, text: &str, config: &ChunkConfig) -> AppResult<Vec<Chunk>> {
        // If tree-sitter not available, fall back to text splitter
        if !self.language.has_tree_sitter_support() {
            tracing::debug!(
                "No tree-sitter support for {:?}, using fallback",
                self.language
            );
            return fallback_split(source_id, text, config, &self.language);
        }

        let ts_language = self
            .language
            .tree_sitter_language()
            .ok_or_else(|| AppError::Other("Tree-sitter language not available".into()))?;

        let mut parser = Parser::new();
        parser
            .set_language(&ts_language)
            .map_err(|e| AppError::Other(format!("Failed to set parser language: {}", e)))?;

        let tree = parser
            .parse(text, None)
            .ok_or_else(|| AppError::Other("Failed to parse code".into()))?;

        // Extract top-level nodes (functions, classes, etc.)
        let chunks = extract_semantic_nodes(source_id, text, &tree, config, &self.language)?;

        tracing::debug!(
            "Code splitter ({:?}) created {} chunks from {} bytes",
            self.language,
            chunks.len(),
            text.len()
        );

        Ok(chunks)
    }
}

/// Extract semantic nodes from tree-sitter parse tree.
fn extract_semantic_nodes(
    source_id: &str,
    text: &str,
    tree: &tree_sitter::Tree,
    config: &ChunkConfig,
    language: &Language,
) -> AppResult<Vec<Chunk>> {
    let root_node = tree.root_node();
    let mut chunks = Vec::new();
    let mut cursor = root_node.walk();

    let mut position = 0u32;

    // Traverse top-level nodes
    for child in root_node.children(&mut cursor) {
        let start_byte = child.start_byte();
        let end_byte = child.end_byte();
        let node_text = &text[start_byte..end_byte];

        // Skip empty or tiny nodes
        if node_text.trim().len() < config.min_chunk_size {
            continue;
        }

        // If node is too large, try to split it further
        if node_text.len() > config.max_chunk_size {
            // For large nodes, fall back to text-based splitting
            let sub_chunks = split_large_node(source_id, node_text, start_byte, config, language)?;
            for mut chunk in sub_chunks {
                chunk.position = position;
                chunks.push(chunk);
                position += 1;
            }
        } else {
            let mut chunk = Chunk::new(
                source_id.to_string(),
                position,
                node_text.to_string(),
                (start_byte, end_byte),
                ContentType::Code {
                    language: language.clone(),
                },
                "code-splitter".to_string(),
            );
            chunk.metadata.language = Some(language.clone());

            chunks.push(chunk);
            position += 1;
        }
    }

    // If no meaningful chunks extracted, fall back to whole file
    if chunks.is_empty() {
        chunks.push(Chunk::new(
            source_id.to_string(),
            0,
            text.to_string(),
            (0, text.len()),
            ContentType::Code {
                language: language.clone(),
            },
            "code-splitter-whole".to_string(),
        ));
    }

    Ok(chunks)
}

/// Split a large code node into smaller chunks.
fn split_large_node(
    source_id: &str,
    text: &str,
    base_offset: usize,
    config: &ChunkConfig,
    language: &Language,
) -> AppResult<Vec<Chunk>> {
    let mut chunks = Vec::new();
    let mut start = 0;

    while start < text.len() {
        let mut end = (start + config.target_chunk_size).min(text.len());

        // Find valid UTF-8 boundary
        while end > start && !text.is_char_boundary(end) {
            end -= 1;
        }

        // Try to break at line boundary
        if end < text.len() {
            if let Some(last_newline) = text[start..end].rfind('\n') {
                end = start + last_newline + 1;
            }
        }

        let chunk_text = text[start..end].trim().to_string();
        if !chunk_text.is_empty() {
            let mut chunk = Chunk::new(
                source_id.to_string(),
                0, // Position will be set by caller
                chunk_text,
                (base_offset + start, base_offset + end),
                ContentType::Code {
                    language: language.clone(),
                },
                "code-splitter-large".to_string(),
            );
            chunk.metadata.language = Some(language.clone());
            chunks.push(chunk);
        }

        start = end;
    }

    Ok(chunks)
}

/// Fallback split when tree-sitter not available.
fn fallback_split(
    source_id: &str,
    text: &str,
    config: &ChunkConfig,
    language: &Language,
) -> AppResult<Vec<Chunk>> {
    use super::FallbackSplitter;

    let splitter = FallbackSplitter;
    let mut chunks = splitter.split(source_id, text, config)?;

    // Update content type to code
    for chunk in &mut chunks {
        chunk.metadata.content_type = ContentType::Code {
            language: language.clone(),
        };
        chunk.metadata.language = Some(language.clone());
    }

    Ok(chunks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_code_splitter() {
        let splitter = CodeSplitter::new(Language::Rust);
        let config = ChunkConfig::default();
        let code = r#"
fn main() {
    println!("Hello, world!");
}

fn another_function() {
    println!("Another function");
}
"#;

        let chunks = splitter.split("test-source", code, &config).unwrap();
        assert!(!chunks.is_empty());
        
        for chunk in &chunks {
            assert!(matches!(chunk.metadata.content_type, ContentType::Code { .. }));
        }
    }

    #[test]
    fn test_unsupported_language_fallback() {
        let splitter = CodeSplitter::new(Language::Unknown);
        let config = ChunkConfig::default();
        let code = "some code here";

        let result = splitter.split("test-source", code, &config);
        assert!(result.is_ok());
    }
}
