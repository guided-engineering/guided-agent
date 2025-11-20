//! Chunk merging and post-processing.

use super::{Chunk, ChunkConfig};

/// Merge consecutive small chunks to reach target size.
pub fn post_process_chunks(chunks: Vec<Chunk>, config: &ChunkConfig) -> Vec<Chunk> {
    if chunks.is_empty() {
        return chunks;
    }

    let mut processed = Vec::new();
    let mut i = 0;

    while i < chunks.len() {
        let mut current = chunks[i].clone();

        // Skip chunks that are too small (unless it's the last chunk)
        if current.text.len() < config.min_chunk_size && i < chunks.len() - 1 {
            i += 1;
            continue;
        }

        // Split oversized chunks
        if current.text.len() > config.max_chunk_size {
            let split_chunks = split_oversized(current, config);
            processed.extend(split_chunks);
            i += 1;
            continue;
        }

        // Try to merge with next chunk if both are small
        if i + 1 < chunks.len() {
            let next = &chunks[i + 1];
            if should_merge(&current, next, config) {
                current = merge_two_chunks(current, next.clone());
                i += 2; // Skip next chunk as it's merged
                processed.push(current);
                continue;
            }
        }

        processed.push(current);
        i += 1;
    }

    // Update positions
    for (pos, chunk) in processed.iter_mut().enumerate() {
        chunk.position = pos as u32;
    }

    processed
}

/// Check if two chunks should be merged.
fn should_merge(chunk1: &Chunk, chunk2: &Chunk, config: &ChunkConfig) -> bool {
    let combined_len = chunk1.text.len() + chunk2.text.len();
    
    // Merge if both are small and combined size is reasonable
    combined_len <= config.target_chunk_size * 2
        && chunk1.text.len() < config.target_chunk_size
        && chunk2.text.len() < config.target_chunk_size
}

/// Merge two chunks into one.
fn merge_two_chunks(mut chunk1: Chunk, chunk2: Chunk) -> Chunk {
    chunk1.text.push('\n');
    chunk1.text.push_str(&chunk2.text);
    chunk1.metadata.byte_range.1 = chunk2.metadata.byte_range.1;
    chunk1.metadata.char_count = chunk1.text.chars().count();
    
    if let (Some(line1), Some(line2)) = (chunk1.metadata.line_range, chunk2.metadata.line_range) {
        chunk1.metadata.line_range = Some((line1.0, line2.1));
    }
    
    chunk1
}

/// Split an oversized chunk into smaller chunks.
fn split_oversized(chunk: Chunk, config: &ChunkConfig) -> Vec<Chunk> {
    let text = &chunk.text;
    let mut result = Vec::new();
    let mut start = 0;
    let mut position = chunk.position;

    while start < text.len() {
        let mut end = (start + config.target_chunk_size).min(text.len());
        
        // Find valid UTF-8 boundary
        while end > start && !text.is_char_boundary(end) {
            end -= 1;
        }
        
        // Try to break at word boundary
        if end < text.len() {
            if let Some(last_space) = text[start..end].rfind(|c: char| c.is_whitespace()) {
                end = start + last_space;
            }
        }

        let chunk_text = text[start..end].trim().to_string();
        if !chunk_text.is_empty() {
            let mut new_chunk = Chunk::new(
                chunk.source_id.clone(),
                position,
                chunk_text,
                (chunk.metadata.byte_range.0 + start, chunk.metadata.byte_range.0 + end),
                chunk.metadata.content_type.clone(),
                chunk.metadata.splitter_used.clone(),
            );
            new_chunk.metadata.language = chunk.metadata.language.clone();
            result.push(new_chunk);
            position += 1;
        }

        start = end;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::detection::ContentType;

    fn create_test_chunk(text: &str, position: u32) -> Chunk {
        Chunk::new(
            "test-source".to_string(),
            position,
            text.to_string(),
            (0, text.len()),
            ContentType::Text,
            "test-splitter".to_string(),
        )
    }

    #[test]
    fn test_merge_small_chunks() {
        let config = ChunkConfig::default();
        let chunks = vec![
            create_test_chunk("Short text", 0),
            create_test_chunk("Another short", 1),
            create_test_chunk("Third short", 2),
        ];

        let processed = post_process_chunks(chunks, &config);
        
        // Should merge some chunks
        assert!(processed.len() < 3);
    }

    #[test]
    fn test_skip_tiny_chunks() {
        let config = ChunkConfig {
            min_chunk_size: 50,
            ..Default::default()
        };
        
        let chunks = vec![
            create_test_chunk("Tiny", 0),
            create_test_chunk("x".repeat(200).as_str(), 1),
        ];

        let processed = post_process_chunks(chunks, &config);
        
        // Tiny chunk should be skipped
        assert_eq!(processed.len(), 1);
    }

    #[test]
    fn test_split_oversized() {
        let config = ChunkConfig {
            target_chunk_size: 100,
            max_chunk_size: 150,
            ..Default::default()
        };
        
        let large_text = "x".repeat(300);
        let chunks = vec![create_test_chunk(&large_text, 0)];

        let processed = post_process_chunks(chunks, &config);
        
        // Should split into multiple chunks
        assert!(processed.len() > 1);
    }
}
