//! Tests for RAG ranking correctness.

use crate::index::{init_index, insert_chunk, insert_source, query_chunks};
use crate::types::{KnowledgeChunk, KnowledgeSource};
use chrono::Utc;
use tempfile::NamedTempFile;

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a test chunk with embedding.
    fn create_test_chunk(
        id: &str,
        source_id: &str,
        text: &str,
        embedding: Vec<f32>,
    ) -> KnowledgeChunk {
        KnowledgeChunk {
            id: id.to_string(),
            source_id: source_id.to_string(),
            position: 0,
            text: text.to_string(),
            embedding: Some(embedding),
            metadata: serde_json::json!({}),
        }
    }

    /// Helper to create a normalized embedding.
    fn normalize(v: &[f32]) -> Vec<f32> {
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            v.iter().map(|x| x / norm).collect()
        } else {
            v.to_vec()
        }
    }

    #[test]
    fn test_relevant_query_returns_high_scores() {
        let temp_file = NamedTempFile::new().unwrap();
        let conn = init_index(temp_file.path()).unwrap();

        // Insert source
        let source = KnowledgeSource {
            id: "source1".to_string(),
            path: None,
            url: None,
            content_type: "text".to_string(),
            learned_at: Utc::now(),
            size_bytes: 100,
        };
        insert_source(&conn, &source).unwrap();

        // Create chunks with similar embeddings to our query
        // Query will be about "rust programming"
        let rust_chunk = create_test_chunk(
            "chunk1",
            "source1",
            "Rust is a systems programming language",
            normalize(&[1.0, 0.5, 0.2, 0.1]),
        );

        let unrelated_chunk = create_test_chunk(
            "chunk2",
            "source1",
            "Cooking recipes for pasta",
            normalize(&[-0.3, -0.8, 0.4, -0.2]),
        );

        insert_chunk(&conn, &rust_chunk).unwrap();
        insert_chunk(&conn, &unrelated_chunk).unwrap();

        // Query with embedding similar to rust_chunk
        let query_embedding = normalize(&[0.9, 0.4, 0.3, 0.1]);
        let results = query_chunks(&conn, &query_embedding, 5).unwrap();

        // Should return both chunks, but rust_chunk with higher score
        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0].0.id, "chunk1",
            "Most relevant chunk should be first"
        );

        // First chunk should have high score (> 0.8)
        assert!(
            results[0].1 > 0.8,
            "Relevant chunk score should be high: {}",
            results[0].1
        );

        // Second chunk should have lower score
        assert!(results[0].1 > results[1].1, "Scores should be ordered");
    }

    #[test]
    fn test_unrelated_query_returns_low_scores() {
        let temp_file = NamedTempFile::new().unwrap();
        let conn = init_index(temp_file.path()).unwrap();

        // Insert source
        let source = KnowledgeSource {
            id: "source1".to_string(),
            path: None,
            url: None,
            content_type: "text".to_string(),
            learned_at: Utc::now(),
            size_bytes: 100,
        };
        insert_source(&conn, &source).unwrap();

        // Create a chunk about programming
        let programming_chunk = create_test_chunk(
            "chunk1",
            "source1",
            "Rust programming language features",
            normalize(&[1.0, 0.0, 0.0, 0.0]),
        );

        insert_chunk(&conn, &programming_chunk).unwrap();

        // Query with completely unrelated embedding (about cooking)
        let query_embedding = normalize(&[0.0, 1.0, 0.0, 0.0]);
        let results = query_chunks(&conn, &query_embedding, 5).unwrap();

        // Should still return the chunk but with low score
        assert_eq!(results.len(), 1);

        // Score should be low (near 0 for orthogonal vectors)
        assert!(
            results[0].1 < 0.5,
            "Unrelated chunk score should be low: {}",
            results[0].1
        );
    }

    #[test]
    fn test_scores_are_ordered_descending() {
        let temp_file = NamedTempFile::new().unwrap();
        let conn = init_index(temp_file.path()).unwrap();

        // Insert source
        let source = KnowledgeSource {
            id: "source1".to_string(),
            path: None,
            url: None,
            content_type: "text".to_string(),
            learned_at: Utc::now(),
            size_bytes: 100,
        };
        insert_source(&conn, &source).unwrap();

        // Create chunks with varying similarity to query
        let chunks = vec![
            create_test_chunk("chunk1", "source1", "Text A", normalize(&[1.0, 0.0, 0.0])),
            create_test_chunk("chunk2", "source1", "Text B", normalize(&[0.7, 0.7, 0.0])),
            create_test_chunk("chunk3", "source1", "Text C", normalize(&[0.0, 1.0, 0.0])),
            create_test_chunk("chunk4", "source1", "Text D", normalize(&[-1.0, 0.0, 0.0])),
        ];

        for chunk in chunks {
            insert_chunk(&conn, &chunk).unwrap();
        }

        // Query with embedding [1, 0, 0]
        let query_embedding = normalize(&[1.0, 0.0, 0.0]);
        let results = query_chunks(&conn, &query_embedding, 10).unwrap();

        // Verify scores are in descending order
        for i in 1..results.len() {
            assert!(
                results[i - 1].1 >= results[i].1,
                "Scores should be ordered: {} >= {}",
                results[i - 1].1,
                results[i].1
            );
        }

        // First chunk should have highest score (closest to [1,0,0])
        assert_eq!(results[0].0.id, "chunk1");
        assert!(
            results[0].1 > 0.99,
            "Perfect match should have score near 1.0"
        );
    }

    #[test]
    fn test_negative_similarity_chunks() {
        let temp_file = NamedTempFile::new().unwrap();
        let conn = init_index(temp_file.path()).unwrap();

        // Insert source
        let source = KnowledgeSource {
            id: "source1".to_string(),
            path: None,
            url: None,
            content_type: "text".to_string(),
            learned_at: Utc::now(),
            size_bytes: 100,
        };
        insert_source(&conn, &source).unwrap();

        // Create chunk opposite to query direction
        let opposite_chunk = create_test_chunk(
            "chunk1",
            "source1",
            "Opposite content",
            normalize(&[-1.0, 0.0, 0.0]),
        );

        insert_chunk(&conn, &opposite_chunk).unwrap();

        // Query with opposite embedding
        let query_embedding = normalize(&[1.0, 0.0, 0.0]);
        let results = query_chunks(&conn, &query_embedding, 5).unwrap();

        // Should return chunk with negative score
        assert_eq!(results.len(), 1);
        assert!(
            results[0].1 < 0.0,
            "Opposite vectors should have negative similarity"
        );
        assert!(
            results[0].1 > -1.1 && results[0].1 < -0.9,
            "Should be close to -1.0"
        );
    }

    #[test]
    fn test_empty_index_returns_no_results() {
        let temp_file = NamedTempFile::new().unwrap();
        let conn = init_index(temp_file.path()).unwrap();

        let query_embedding = normalize(&[1.0, 0.0, 0.0]);
        let results = query_chunks(&conn, &query_embedding, 5).unwrap();

        assert_eq!(results.len(), 0, "Empty index should return no results");
    }

    #[test]
    fn test_top_k_limit_respected() {
        let temp_file = NamedTempFile::new().unwrap();
        let conn = init_index(temp_file.path()).unwrap();

        // Insert source
        let source = KnowledgeSource {
            id: "source1".to_string(),
            path: None,
            url: None,
            content_type: "text".to_string(),
            learned_at: Utc::now(),
            size_bytes: 100,
        };
        insert_source(&conn, &source).unwrap();

        // Insert 10 chunks
        for i in 0..10 {
            let chunk = create_test_chunk(
                &format!("chunk{}", i),
                "source1",
                &format!("Text {}", i),
                normalize(&[i as f32 / 10.0, 0.0, 0.0]),
            );
            insert_chunk(&conn, &chunk).unwrap();
        }

        // Query with top_k = 3
        let query_embedding = normalize(&[1.0, 0.0, 0.0]);
        let results = query_chunks(&conn, &query_embedding, 3).unwrap();

        assert_eq!(results.len(), 3, "Should return exactly top_k results");
    }
}
