//! Text splitter using text-splitter crate.

use super::ChunkSplitter;
use crate::chunk::{detection::ContentType, Chunk, ChunkConfig};
use guided_core::AppResult;
use text_splitter::TextSplitter as ExternalTextSplitter;

pub struct TextSplitter;

impl ChunkSplitter for TextSplitter {
    fn split(&self, source_id: &str, text: &str, config: &ChunkConfig) -> AppResult<Vec<Chunk>> {
        // Use text-splitter crate for semantic splitting
        let splitter = ExternalTextSplitter::new(config.target_chunk_size);
        
        let raw_chunks: Vec<&str> = splitter.chunks(text).collect();

        let mut chunks = Vec::new();
        let mut byte_offset = 0;

        for (position, chunk_text) in raw_chunks.iter().enumerate() {
            if chunk_text.trim().is_empty() {
                continue;
            }

            let chunk_len = chunk_text.len();
            let chunk = Chunk::new(
                source_id.to_string(),
                position as u32,
                chunk_text.to_string(),
                (byte_offset, byte_offset + chunk_len),
                ContentType::Text,
                "text-splitter".to_string(),
            );

            chunks.push(chunk);
            byte_offset += chunk_len;
        }

        tracing::debug!(
            "Text splitter created {} chunks from {} bytes",
            chunks.len(),
            text.len()
        );

        Ok(chunks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_splitter_basic() {
        let splitter = TextSplitter;
        let config = ChunkConfig::default();
        let text = "This is a test. ".repeat(100);

        let chunks = splitter.split("test-source", &text, &config).unwrap();
        assert!(!chunks.is_empty());
        
        for chunk in &chunks {
            assert!(!chunk.text.is_empty());
            assert_eq!(chunk.source_id, "test-source");
        }
    }

    #[test]
    fn test_text_splitter_utf8() {
        let splitter = TextSplitter;
        let config = ChunkConfig::default();
        let text = "Gamedex é um aplicativo 🎮 com acentuação: ã, õ, ç. ".repeat(50);

        let result = splitter.split("test-source", &text, &config);
        assert!(result.is_ok());
        
        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_text_splitter_with_overlap() {
        let splitter = TextSplitter;
        let mut config = ChunkConfig::default();
        config.target_chunk_size = 100;
        config.overlap = 20;

        let text = "a".repeat(500);
        let chunks = splitter.split("test-source", &text, &config).unwrap();
        
        assert!(chunks.len() > 1);
    }
}
