//! Knowledge base management system.
//!
//! Provides local-first RAG using LanceDB vector index.

pub mod chunk;
pub mod chunker; // Deprecated: use chunk module instead
pub mod config;
pub mod embeddings;
pub mod lancedb_index;
pub mod parser;
pub mod rag;
pub mod types;
pub mod vector_index;

#[cfg(test)]
mod tests;

// Re-export commonly used types
pub use rag::{RagResponse, RagSourceRef};
pub use types::{
    AskOptions, AskResult, BaseStats, KnowledgeBaseConfig, KnowledgeChunk, KnowledgeSource,
    LearnOptions, LearnStats,
};

use guided_core::{AppError, AppResult};
use std::path::Path;
use std::time::Instant;

use walkdir::WalkDir;

/// Minimum cosine similarity score for a chunk to be considered relevant.
/// Scores below this threshold will be filtered out.
/// Range: -1.0 to 1.0, where 1.0 is perfect match, 0.0 is orthogonal, -1.0 is opposite.
/// Note: 0.20 is suitable for mock embeddings; production systems should use 0.3-0.5.
const MIN_RELEVANCE_SCORE: f32 = 0.20;

/// Learn from sources and populate the knowledge base.
pub async fn learn(
    workspace: &Path,
    options: &LearnOptions,
    _api_key: Option<&str>,
) -> AppResult<LearnStats> {
    let start = Instant::now();

    tracing::info!("Starting learn operation for base '{}'", options.base_name);

    // Load or create config
    let config = config::load_config(workspace, &options.base_name)?;

    // Initialize LanceDB index
    let index_path = config::get_index_path(workspace, &options.base_name);
    let mut index =
        lancedb_index::LanceDbIndex::new(&index_path, "chunks", config.embedding_dim as usize)
            .await?;

    // Initialize source manager
    let source_manager = rag::SourceManager::new(workspace, &options.base_name);

    // Reset if requested
    if options.reset {
        tracing::info!("Resetting knowledge base");
        use vector_index::VectorIndex;
        index.reset()?;
        source_manager.clear_sources()?;
    }

    let mut sources_count = 0u32;
    let mut chunks_count = 0u32;
    let mut bytes_processed = 0u64;

    // Process paths
    for path in &options.paths {
        if path.is_file() {
            if let Ok((source_id, chunk_count, byte_count)) =
                process_file(workspace, &options.base_name, &mut index, &config, path, &options).await
            {
                // Track source
                let source = KnowledgeSource {
                    source_id,
                    path: path.to_string_lossy().to_string(),
                    source_type: "file".to_string(),
                    indexed_at: chrono::Utc::now(),
                    chunk_count,
                    byte_count,
                };
                source_manager.track_source(&source)?;

                sources_count += 1;
                chunks_count += chunk_count;
                bytes_processed += byte_count;
            }
        } else if path.is_dir() {
            for entry in WalkDir::new(path)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let entry_path = entry.path();
                if entry_path.is_file() && should_include(entry_path, &options) {
                    if let Ok((source_id, chunk_count, byte_count)) =
                        process_file(workspace, &options.base_name, &mut index, &config, entry_path, &options)
                            .await
                    {
                        // Track source
                        let source = KnowledgeSource {
                            source_id,
                            path: entry_path.to_string_lossy().to_string(),
                            source_type: "file".to_string(),
                            indexed_at: chrono::Utc::now(),
                            chunk_count,
                            byte_count,
                        };
                        source_manager.track_source(&source)?;

                        sources_count += 1;
                        chunks_count += chunk_count;
                        bytes_processed += byte_count;
                    }
                }
            }
        }
    }

    // Flush index
    use vector_index::VectorIndex;
    index.flush()?;

    // Save config
    config::save_config(workspace, &config)?;

    let duration = start.elapsed();

    tracing::info!(
        "Learn operation completed: {} sources, {} chunks, {} bytes in {:.2}s",
        sources_count,
        chunks_count,
        bytes_processed,
        duration.as_secs_f64()
    );

    Ok(LearnStats {
        sources_count,
        chunks_count,
        bytes_processed,
        duration_secs: duration.as_secs_f64(),
    })
}

/// Process a single file.
/// Returns (source_id, chunk_count, byte_count).
async fn process_file(
    workspace: &Path,
    base_name: &str,
    index: &mut dyn vector_index::VectorIndex,
    config: &KnowledgeBaseConfig,
    path: &Path,
    _options: &LearnOptions,
) -> AppResult<(String, u32, u64)> {
    tracing::debug!("Processing file: {:?}", path);

    // Parse file
    let text = parser::parse_file(path)?;
    let size_bytes = text.len() as u64;

    // Create source
    let source_id = uuid::Uuid::new_v4().to_string();

    // Use new hybrid chunking pipeline
    let chunk_config = chunk::ChunkConfig {
        target_chunk_size: config.chunk_size as usize,
        max_chunk_size: (config.chunk_size * 2) as usize,
        min_chunk_size: (config.chunk_size / 10) as usize,
        overlap: config.chunk_overlap as usize,
        respect_semantics: true,
        preserve_code_blocks: true,
    };
    
    let pipeline = chunk::ChunkPipeline::new(chunk_config);
    let mut chunks = pipeline.process(&source_id, &text, Some(path))?;

    // Enrich metadata with source path
    for chunk_item in &mut chunks {
        if let Some(custom) = chunk_item.metadata.custom.as_object_mut() {
            custom.insert("source_path".to_string(), serde_json::json!(path.to_string_lossy()));
        } else {
            let mut custom_map = serde_json::Map::new();
            custom_map.insert("source_path".to_string(), serde_json::json!(path.to_string_lossy()));
            chunk_item.metadata.custom = serde_json::Value::Object(custom_map);
        }
    }

    // Use EmbeddingEngine for batch embedding
    let engine = crate::embeddings::EmbeddingEngine::new(workspace.to_path_buf());
    let embeddings = engine.embed_chunks(base_name, &chunks, None).await?;
    
    let chunks_count = chunks.len() as u32;

    // Convert to KnowledgeChunk with embeddings and insert
    for (chunk_item, embedding) in chunks.into_iter().zip(embeddings) {
        let knowledge_chunk = KnowledgeChunk {
            id: chunk_item.id,
            source_id: chunk_item.source_id,
            position: chunk_item.position,
            text: chunk_item.text,
            embedding: Some(embedding),
            metadata: serde_json::to_value(&chunk_item.metadata)?,
        };

        index.upsert_chunk(&knowledge_chunk)?;
    }

    tracing::debug!(
        "Processed {:?}: {} chunks, {} bytes",
        path,
        chunks_count,
        size_bytes
    );

    Ok((source_id, chunks_count, size_bytes))
}

/// Check if a file should be included based on patterns.
fn should_include(path: &Path, options: &LearnOptions) -> bool {
    let path_str = path.to_string_lossy();

    // Check excludes first
    for pattern in &options.exclude {
        if path_str.contains(pattern) {
            return false;
        }
    }

    // If includes are specified, must match at least one
    if !options.include.is_empty() {
        for pattern in &options.include {
            if path_str.contains(pattern) {
                return true;
            }
        }
        return false;
    }

    true
}

/// Query the knowledge base and return relevant chunks.
pub async fn ask(
    workspace: &Path,
    options: AskOptions,
    api_key: Option<&str>,
) -> AppResult<AskResult> {
    tracing::info!(
        "Querying knowledge base '{}' with query: {}",
        options.base_name,
        options.query
    );

    // Load config
    let config = config::load_config(workspace, &options.base_name)?;

    // Check if index exists
    let index_path = config::get_index_path(workspace, &options.base_name);
    if !index_path.exists() {
        return Err(AppError::Knowledge(format!(
            "Knowledge base '{}' has no index. Run 'guided knowledge learn' first.",
            options.base_name
        )));
    }

    // Initialize LanceDB index
    let index =
        lancedb_index::LanceDbIndex::new(&index_path, "chunks", config.embedding_dim as usize)
            .await?;

    // Generate query embedding using EmbeddingEngine
    let engine = crate::embeddings::EmbeddingEngine::new(workspace.to_path_buf());
    let query_embeddings = engine.embed_texts(&options.base_name, &[options.query.clone()], api_key).await?;
    let query_embedding = query_embeddings.into_iter().next().ok_or_else(|| {
        AppError::Knowledge("Failed to generate query embedding".to_string())
    })?;

    // Retrieve top-k chunks
    use vector_index::VectorIndex;
    let results = index.search(&query_embedding, options.top_k as usize)?;

    // Debug: log scores before filtering
    if !results.is_empty() {
        let all_scores: Vec<f32> = results.iter().map(|(_, s)| *s).collect();
        tracing::debug!(
            "Retrieved {} chunks before filtering - scores: {:?}",
            results.len(),
            all_scores
        );
    }

    // Apply relevance cutoff - filter out chunks with low similarity
    let filtered_results: Vec<_> = results
        .into_iter()
        .filter(|(_chunk, score)| *score >= MIN_RELEVANCE_SCORE)
        .collect();

    let chunks: Vec<KnowledgeChunk> = filtered_results
        .iter()
        .map(|(chunk, _score)| chunk.clone())
        .collect();
    let scores: Vec<f32> = filtered_results
        .iter()
        .map(|(_chunk, score)| *score)
        .collect();

    if chunks.is_empty() {
        tracing::info!(
            "No relevant chunks found (all scores below {:.2} threshold)",
            MIN_RELEVANCE_SCORE
        );
    } else {
        tracing::info!(
            "Retrieved {} relevant chunks (top score: {:.3}, lowest: {:.3})",
            chunks.len(),
            scores.first().unwrap_or(&0.0),
            scores.last().unwrap_or(&0.0)
        );
    }

    Ok(AskResult { chunks, scores })
}

/// Clean (reset) a knowledge base.
pub async fn clean(workspace: &Path, base_name: &str) -> AppResult<()> {
    tracing::info!("Cleaning knowledge base '{}'", base_name);

    let config = config::load_config(workspace, base_name)?;
    let index_path = config::get_index_path(workspace, base_name);

    if !index_path.exists() {
        return Err(AppError::Knowledge(format!(
            "Knowledge base '{}' does not exist",
            base_name
        )));
    }

    let mut index =
        lancedb_index::LanceDbIndex::new(&index_path, "chunks", config.embedding_dim as usize)
            .await?;

    use vector_index::VectorIndex;
    index.reset()?;

    // Clear source tracking
    let source_manager = rag::SourceManager::new(workspace, base_name);
    source_manager.clear_sources()?;

    tracing::info!("Knowledge base '{}' cleaned (index and sources.jsonl cleared)", base_name);
    Ok(())
}

/// Get statistics for a knowledge base.
pub async fn stats(workspace: &Path, base_name: &str) -> AppResult<BaseStats> {
    tracing::info!("Getting stats for knowledge base '{}'", base_name);

    let config = config::load_config(workspace, base_name)?;
    let index_path = config::get_index_path(workspace, base_name);

    if !index_path.exists() {
        return Err(AppError::Knowledge(format!(
            "Knowledge base '{}' does not exist",
            base_name
        )));
    }

    let index =
        lancedb_index::LanceDbIndex::new(&index_path, "chunks", config.embedding_dim as usize)
            .await?;

    use vector_index::VectorIndex;
    let (sources_count, chunks_count) = index.stats()?;

    // Calculate directory size
    let db_size_bytes = calculate_dir_size(&index_path);

    // Read sources.jsonl to get last_learn_at
    let source_manager = rag::SourceManager::new(workspace, base_name);
    let sources = source_manager.list_sources().unwrap_or_default();
    
    let last_learn_at = sources
        .iter()
        .map(|s| s.indexed_at)
        .max();

    tracing::debug!(
        "Stats for '{}': {} sources, {} chunks, {} bytes, last_learn_at: {:?}",
        base_name,
        sources_count,
        chunks_count,
        db_size_bytes,
        last_learn_at
    );

    Ok(BaseStats {
        base_name: base_name.to_string(),
        sources_count,
        chunks_count,
        db_size_bytes,
        last_learn_at,
    })
}

/// Calculate total size of a directory recursively.
fn calculate_dir_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.metadata().ok())
        .filter(|metadata| metadata.is_file())
        .map(|metadata| metadata.len())
        .sum()
}
