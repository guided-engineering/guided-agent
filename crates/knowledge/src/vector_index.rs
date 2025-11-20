//! Vector index abstraction for knowledge chunks.
//!
//! Defines a trait for provider-agnostic vector storage and retrieval.

use crate::types::KnowledgeChunk;
use guided_core::AppResult;

/// Trait for vector index backends.
///
/// Implementations must support:
/// - Upserting chunks with embeddings
/// - Searching for similar vectors (top-k)
/// - Collecting statistics
/// - Resetting/clearing the index
pub trait VectorIndex: Send + Sync {
    /// Insert or update a chunk with its embedding in the index.
    fn upsert_chunk(&mut self, chunk: &KnowledgeChunk) -> AppResult<()>;

    /// Search for the top-k most similar chunks to the query embedding.
    ///
    /// Returns chunks ordered by descending similarity score.
    fn search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> AppResult<Vec<(KnowledgeChunk, f32)>>;

    /// Get statistics about the index.
    ///
    /// Returns (sources_count, chunks_count).
    fn stats(&self) -> AppResult<(u32, u32)>;

    /// Reset the index, removing all chunks and sources.
    fn reset(&mut self) -> AppResult<()>;

    /// Commit any pending changes (for backends that buffer writes).
    fn flush(&mut self) -> AppResult<()> {
        // Default implementation does nothing
        Ok(())
    }
}
