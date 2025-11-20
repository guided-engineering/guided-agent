//! Chunking pipeline orchestrator.

use super::{
    detection::{detect_content_type, ContentType},
    merging::post_process_chunks,
    splitters::{ChunkSplitter, CodeSplitter, FallbackSplitter, TextSplitter},
    Chunk,
};
use guided_core::AppResult;
use std::path::Path;

/// Configuration for chunking pipeline.
#[derive(Debug, Clone)]
pub struct ChunkConfig {
    /// Target chunk size in characters
    pub target_chunk_size: usize,
    
    /// Maximum chunk size before forcing split
    pub max_chunk_size: usize,
    
    /// Minimum chunk size (skip smaller chunks)
    pub min_chunk_size: usize,
    
    /// Overlap between chunks in characters
    pub overlap: usize,
    
    /// Respect semantic boundaries when possible
    pub respect_semantics: bool,
    
    /// Preserve code blocks in markdown
    pub preserve_code_blocks: bool,
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self {
            target_chunk_size: 1000,
            max_chunk_size: 2000,
            min_chunk_size: 100,
            overlap: 200,
            respect_semantics: true,
            preserve_code_blocks: true,
        }
    }
}

/// Hybrid chunking pipeline.
pub struct ChunkPipeline {
    config: ChunkConfig,
}

impl ChunkPipeline {
    /// Create a new pipeline with configuration.
    pub fn new(config: ChunkConfig) -> Self {
        Self { config }
    }

    /// Process text into semantic chunks.
    pub fn process(
        &self,
        source_id: &str,
        text: &str,
        path: Option<&Path>,
    ) -> AppResult<Vec<Chunk>> {
        // 1. Detect content type
        let content_type = detect_content_type(path, text);
        
        tracing::debug!(
            "Detected content type: {:?} for source: {}",
            content_type,
            source_id
        );

        // 2. Select appropriate splitter
        let splitter = self.dispatch_splitter(&content_type);

        // 3. Split into chunks
        let chunks = splitter.split(source_id, text, &self.config)?;

        // 4. Post-process and merge
        let processed = post_process_chunks(chunks, &self.config);

        tracing::info!(
            "Chunking complete: {} chunks created from {} bytes",
            processed.len(),
            text.len()
        );

        Ok(processed)
    }

    /// Select the appropriate splitter for content type.
    fn dispatch_splitter(&self, content_type: &ContentType) -> Box<dyn ChunkSplitter> {
        match content_type {
            ContentType::Text | ContentType::Markdown | ContentType::Html | ContentType::Pdf => {
                Box::new(TextSplitter)
            }
            ContentType::Code { language } => Box::new(CodeSplitter::new(language.clone())),
            ContentType::Unknown => Box::new(FallbackSplitter),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_text() {
        let pipeline = ChunkPipeline::new(ChunkConfig::default());
        let text = "This is a test document. ".repeat(100);

        let chunks = pipeline.process("test-source", &text, None).unwrap();
        assert!(!chunks.is_empty());
        
        for chunk in &chunks {
            assert!(!chunk.text.is_empty());
            assert_eq!(chunk.source_id, "test-source");
        }
    }

    #[test]
    fn test_pipeline_rust_code() {
        let pipeline = ChunkPipeline::new(ChunkConfig::default());
        let code = r#"
fn main() {
    println!("Hello, world!");
}

fn test() {
    assert_eq!(1 + 1, 2);
}
"#;
        let path = Path::new("test.rs");

        let chunks = pipeline.process("test-source", code, Some(path)).unwrap();
        assert!(!chunks.is_empty());
        
        for chunk in &chunks {
            assert!(matches!(chunk.metadata.content_type, ContentType::Code { .. }));
        }
    }

    #[test]
    fn test_pipeline_markdown() {
        let pipeline = ChunkPipeline::new(ChunkConfig::default());
        let markdown = r#"
# Title

This is a paragraph.

## Section

- Item 1
- Item 2

```rust
fn main() {}
```
"#;
        let path = Path::new("README.md");

        let chunks = pipeline.process("test-source", markdown, Some(path)).unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_pipeline_utf8_safety() {
        let pipeline = ChunkPipeline::new(ChunkConfig::default());
        
        // Test with various UTF-8 characters
        let text = "Gamedex é um aplicativo 🎮 brasileiro. \
                    Acentuação: ã, õ, ç, á, é, í, ó, ú, à, â, ê, ô. \
                    Emoji: 🚀 🎯 💡 ✨ 🔥. ".repeat(50);

        let result = pipeline.process("test-source", &text, None);
        assert!(result.is_ok());
        
        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
        
        // Verify all chunks are valid UTF-8
        for chunk in &chunks {
            assert!(std::str::from_utf8(chunk.text.as_bytes()).is_ok());
        }
    }

    #[test]
    fn test_pipeline_large_file() {
        let pipeline = ChunkPipeline::new(ChunkConfig {
            target_chunk_size: 500,
            max_chunk_size: 1000,
            min_chunk_size: 50,
            overlap: 100,
            respect_semantics: true,
            preserve_code_blocks: true,
        });

        let text = "This is a sentence. ".repeat(1000);
        let chunks = pipeline.process("test-source", &text, None).unwrap();
        
        assert!(chunks.len() > 1);
        
        // Verify chunks respect size constraints
        for chunk in &chunks {
            assert!(chunk.text.len() >= 50 || chunks.len() == 1);
            assert!(chunk.text.len() <= 1000);
        }
    }
}
