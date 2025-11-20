//! Fallback splitter with Unicode-aware splitting.

use super::ChunkSplitter;
use crate::chunk::{detection::ContentType, Chunk, ChunkConfig};
use guided_core::AppResult;


pub struct FallbackSplitter;

impl ChunkSplitter for FallbackSplitter {
    fn split(&self, source_id: &str, text: &str, config: &ChunkConfig) -> AppResult<Vec<Chunk>> {
        let mut chunks = Vec::new();
        let mut start = 0;
        let mut position = 0u32;

        while start < text.len() {
            // Find end position respecting Unicode grapheme boundaries
            let mut end = (start + config.target_chunk_size).min(text.len());

            // Ensure we're at a grapheme boundary
            while end > start && !text.is_char_boundary(end) {
                end -= 1;
            }

            // Try to break at word boundary for better semantics
            if end < text.len() {
                if let Some(last_space) = text[start..end].rfind(|c: char| c.is_whitespace()) {
                    end = start + last_space;
                }
            }

            let chunk_text = text[start..end].trim().to_string();

            // Skip empty chunks
            if chunk_text.is_empty() {
                break;
            }

            // Skip chunks that are too small (unless it's the last chunk)
            if chunk_text.len() < config.min_chunk_size && end < text.len() {
                // Try to extend to next word boundary
                if let Some(next_space) = text[end..].find(|c: char| c.is_whitespace()) {
                    end = end + next_space;
                } else {
                    end = text.len();
                }
                let extended_text = text[start..end].trim().to_string();
                if !extended_text.is_empty() {
                    let chunk = create_chunk(
                        source_id,
                        position,
                        extended_text,
                        (start, end),
                    );
                    chunks.push(chunk);
                    position += 1;
                }
                break;
            }

            let chunk = create_chunk(
                source_id,
                position,
                chunk_text,
                (start, end),
            );
            chunks.push(chunk);
            position += 1;

            // Move forward with overlap
            let step = if config.target_chunk_size > config.overlap {
                config.target_chunk_size - config.overlap
            } else {
                config.target_chunk_size
            };

            start += step;

            // Ensure we're at a grapheme boundary
            while start < text.len() && !text.is_char_boundary(start) {
                start += 1;
            }
        }

        tracing::debug!(
            "Fallback splitter created {} chunks from {} bytes",
            chunks.len(),
            text.len()
        );

        Ok(chunks)
    }
}

fn create_chunk(
    source_id: &str,
    position: u32,
    text: String,
    byte_range: (usize, usize),
) -> Chunk {
    Chunk::new(
        source_id.to_string(),
        position,
        text,
        byte_range,
        ContentType::Unknown,
        "fallback".to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_splitter_basic() {
        let splitter = FallbackSplitter;
        let config = ChunkConfig::default();
        let text = "This is a test. ".repeat(100);

        let chunks = splitter.split("test-source", &text, &config).unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_fallback_splitter_utf8() {
        let splitter = FallbackSplitter;
        let config = ChunkConfig::default();
        
        // Text with emojis, accents, and special characters
        let text = "Gamedex é um aplicativo 🎮 brasileiro com acentuação completa: ã, õ, ç, á, é, í, ó, ú. ".repeat(50);

        let result = splitter.split("test-source", &text, &config);
        assert!(result.is_ok());
        
        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
        
        // Verify no panics and all chunks are valid UTF-8
        for chunk in &chunks {
            assert!(!chunk.text.is_empty());
            assert!(chunk.text.is_char_boundary(0));
            assert!(chunk.text.is_char_boundary(chunk.text.len()));
        }
    }

    #[test]
    fn test_fallback_splitter_with_overlap() {
        let splitter = FallbackSplitter;
        let mut config = ChunkConfig::default();
        config.target_chunk_size = 100;
        config.overlap = 20;
        config.min_chunk_size = 10; // Lower min size for test

        let text = "word ".repeat(200);
        let chunks = splitter.split("test-source", &text, &config).unwrap();
        
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_fallback_splitter_min_size() {
        let splitter = FallbackSplitter;
        let mut config = ChunkConfig::default();
        config.min_chunk_size = 50;

        let text = "Short. ";
        let chunks = splitter.split("test-source", &text, &config).unwrap();
        
        // Should be empty or extended to meet min size
        for chunk in &chunks {
            assert!(chunk.text.len() >= config.min_chunk_size || chunks.len() == 1);
        }
    }
}
