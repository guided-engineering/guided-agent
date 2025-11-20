//! Text chunking with configurable size and overlap.

use crate::types::ChunkCandidate;

/// Chunk text into overlapping segments.
///
/// This implementation uses simple character-based chunking.
/// Future improvements could use token-based chunking with a tokenizer.
pub fn chunk_text(
    source_id: &str,
    text: &str,
    chunk_size: usize,
    overlap: usize,
) -> Vec<ChunkCandidate> {
    if text.is_empty() {
        return vec![];
    }

    let mut chunks = Vec::new();
    let mut position = 0u32;
    let mut start = 0;

    while start < text.len() {
        // Find valid UTF-8 boundary for end position
        let mut end = (start + chunk_size).min(text.len());
        while end > start && !text.is_char_boundary(end) {
            end -= 1;
        }
        
        let chunk_text = &text[start..end];

        // Skip chunks that are too small (< 10% of chunk_size)
        if chunk_text.len() < chunk_size / 10 {
            break;
        }

        chunks.push(ChunkCandidate {
            source_id: source_id.to_string(),
            position,
            text: chunk_text.trim().to_string(),
            metadata: serde_json::json!({
                "start": start,
                "end": end,
            }),
        });

        position += 1;

        // Move forward by (chunk_size - overlap)
        let step = if chunk_size > overlap {
            chunk_size - overlap
        } else {
            chunk_size
        };

        // Find valid UTF-8 boundary for next start position
        let mut next_start = start + step;
        while next_start < text.len() && !text.is_char_boundary(next_start) {
            next_start += 1;
        }
        start = next_start;
    }

    tracing::debug!(
        "Chunked text into {} chunks (size: {}, overlap: {})",
        chunks.len(),
        chunk_size,
        overlap
    );

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_text_basic() {
        let text = "a".repeat(1000);
        let chunks = chunk_text("test-source", &text, 200, 50);

        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].position, 0);
        assert_eq!(chunks[1].position, 1);
    }

    #[test]
    fn test_chunk_text_no_overlap() {
        let text = "a".repeat(300);
        let chunks = chunk_text("test-source", &text, 100, 0);

        assert_eq!(chunks.len(), 3);
    }

    #[test]
    fn test_chunk_text_empty() {
        let chunks = chunk_text("test-source", "", 100, 10);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_chunk_text_with_overlap() {
        let text = "abcdefghijklmnopqrstuvwxyz".repeat(10);
        let chunks = chunk_text("test-source", &text, 50, 10);

        // Verify overlap exists
        if chunks.len() >= 2 {
            let first_end = chunks[0].text.chars().rev().take(10).collect::<String>();
            let second_start = chunks[1].text.chars().take(10).collect::<String>();

            // Some characters should overlap
            assert!(
                first_end.chars().any(|c| second_start.contains(c)),
                "Expected overlap between chunks"
            );
        }
    }
}
