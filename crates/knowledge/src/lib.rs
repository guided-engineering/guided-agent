//! Knowledge base management system.
//!
//! Provides local-first RAG using LanceDB vector index.

pub mod chunk;
pub mod chunker; // Deprecated: use chunk module instead
pub mod config;
pub mod embeddings;
pub mod lancedb_index;
pub mod metadata;
pub mod parser;
pub mod progress;
pub mod rag;
pub mod types;
pub mod vector_index;

#[cfg(test)]
mod tests;

// Re-export commonly used types
pub use progress::{ProgressEvent, ProgressReporter};
pub use rag::{RagResponse, RagSourceRef};
pub use types::{
    AskOptions, AskResult, BaseStats, KnowledgeBaseConfig, KnowledgeChunk, KnowledgeSource,
    LearnOptions, LearnStats,
};

use guided_core::{AppError, AppResult};
use std::path::{Path, PathBuf};
use std::time::Instant;

use walkdir::WalkDir;

/// Minimum cosine similarity score for a chunk to be considered relevant.
/// Scores below this threshold will be filtered out.
/// Range: -1.0 to 1.0, where 1.0 is perfect match, 0.0 is orthogonal, -1.0 is opposite.
/// Note: 0.08 is suitable for trigram embeddings (lower semantic accuracy);
/// production systems with neural embeddings should use 0.3-0.5.
const MIN_RELEVANCE_SCORE: f32 = 0.08;

/// Learn from sources and populate the knowledge base.
pub async fn learn(
    workspace: &Path,
    options: &LearnOptions,
    _api_key: Option<&str>,
) -> AppResult<LearnStats> {
    learn_with_progress(workspace, options, _api_key, progress::ProgressReporter::noop()).await
}

/// Learn with progress reporting.
pub async fn learn_with_progress(
    workspace: &Path,
    options: &LearnOptions,
    _api_key: Option<&str>,
    progress: progress::ProgressReporter,
) -> AppResult<LearnStats> {
    let start = Instant::now();

    tracing::info!("Starting learn operation for base '{}'", options.base_name);

    // Load or create config
    let mut config = config::load_config(workspace, &options.base_name)?;

    // Override provider/model if specified in options
    if let Some(provider) = &options.provider {
        config.provider = provider.clone();
        tracing::info!("Using provider from options: {}", provider);
    }
    if let Some(model) = &options.model {
        config.model = model.clone();
        tracing::info!("Using model from options: {}", model);
    }

    // Save config (creates base directory if needed)
    config::save_config(workspace, &config)?;

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

    // Phase 1: Discover files
    let mut all_files = Vec::new();
    for path in &options.paths {
        if path.is_file() {
            all_files.push(path.clone());
        } else if path.is_dir() {
            for entry in WalkDir::new(path).follow_links(false).into_iter().filter_map(|e| e.ok()) {
                let entry_path = entry.path();
                if entry_path.is_file() && should_include(entry_path, &options) {
                    all_files.push(entry_path.to_path_buf());
                }
            }
        }
    }
    
    let total_files = all_files.len() as u64;
    tracing::info!("Discovered {} files to process", total_files);
    
    // Phase 2: Process files with batch optimization
    const BATCH_SIZE: usize = 10; // Process 10 files before embedding batch
    let mut pending_chunks: Vec<(String, Vec<chunk::Chunk>, PathBuf, u64)> = Vec::new();
    
    for (idx, path) in all_files.iter().enumerate() {
        let current = (idx + 1) as u64;
        
        progress.parse(current, Some(total_files), &path.to_string_lossy());
        
        // Parse and chunk file (fast operations)
        match parse_and_chunk_file(workspace, &config, path, &progress).await {
            Ok((source_id, chunks, byte_count)) => {
                pending_chunks.push((source_id.clone(), chunks, path.clone(), byte_count));
                
                // Process batch when full or at end
                if pending_chunks.len() >= BATCH_SIZE || idx == all_files.len() - 1 {
                    let batch_result = process_batch(
                        workspace,
                        &options.base_name,
                        &mut index,
                        &config,
                        &source_manager,
                        &mut pending_chunks,
                        &progress,
                    ).await;
                    
                    match batch_result {
                        Ok((batch_sources, batch_chunks, batch_bytes)) => {
                            sources_count += batch_sources;
                            chunks_count += batch_chunks;
                            bytes_processed += batch_bytes;
                        }
                        Err(e) => {
                            tracing::warn!("Failed to process batch: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to parse/chunk file {:?}: {}", path, e);
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

/// Parse and chunk a file (no embedding yet).
/// Returns (source_id, chunks, byte_count).
async fn parse_and_chunk_file(
    workspace: &Path,
    config: &KnowledgeBaseConfig,
    path: &Path,
    progress: &progress::ProgressReporter,
) -> AppResult<(String, Vec<chunk::Chunk>, u64)> {
    // Parse file
    let text = parser::parse_file(path)?;
    let size_bytes = text.len() as u64;

    // Extract rich metadata using Phase 5.5.1 metadata module
    let file_metadata = metadata::extract_metadata(path, &text);

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

    // Enrich chunks with rich metadata from Phase 5.5.1
    for chunk_item in &mut chunks {
        let mut custom_map = if let Some(custom) = chunk_item.metadata.custom.as_object() {
            custom.clone()
        } else {
            serde_json::Map::new()
        };

        // Add structured metadata fields
        custom_map.insert("source_path".to_string(), serde_json::json!(file_metadata.source_path));
        custom_map.insert("file_name".to_string(), serde_json::json!(file_metadata.file_name));
        custom_map.insert("file_type".to_string(), serde_json::json!(file_metadata.file_type.as_str()));
        if let Some(ref lang) = file_metadata.language {
            custom_map.insert("language".to_string(), serde_json::json!(lang.as_str()));
        }
        custom_map.insert("file_size_bytes".to_string(), serde_json::json!(file_metadata.file_size_bytes));
        custom_map.insert("file_line_count".to_string(), serde_json::json!(file_metadata.file_line_count));
        custom_map.insert("file_modified_at".to_string(), serde_json::json!(file_metadata.file_modified_at.timestamp()));
        custom_map.insert("content_hash".to_string(), serde_json::json!(file_metadata.content_hash));
        custom_map.insert("tags".to_string(), serde_json::json!(file_metadata.tags));
        custom_map.insert("created_at".to_string(), serde_json::json!(file_metadata.created_at.timestamp()));
        custom_map.insert("updated_at".to_string(), serde_json::json!(file_metadata.updated_at.timestamp()));

        chunk_item.metadata.custom = serde_json::Value::Object(custom_map);
    }

    let chunks_count = chunks.len() as u32;
    progress.chunk(1, Some(1), chunks_count);

    Ok((source_id, chunks, size_bytes))
}

/// Process a batch of files: embed all chunks at once and insert in batch.
async fn process_batch(
    workspace: &Path,
    base_name: &str,
    index: &mut dyn vector_index::VectorIndex,
    config: &KnowledgeBaseConfig,
    source_manager: &rag::SourceManager,
    pending: &mut Vec<(String, Vec<chunk::Chunk>, PathBuf, u64)>,
    progress: &progress::ProgressReporter,
) -> AppResult<(u32, u32, u64)> {
    if pending.is_empty() {
        return Ok((0, 0, 0));
    }

    // Collect all chunks from all files in batch
    let mut all_chunks = Vec::new();
    let mut chunk_to_source: Vec<usize> = Vec::new(); // Maps chunk index to source index
    
    for (idx, (_source_id, chunks, _path, _bytes)) in pending.iter().enumerate() {
        for _ in chunks {
            chunk_to_source.push(idx);
        }
        all_chunks.extend(chunks.clone());
    }

    let total_chunks = all_chunks.len();
    
    // Batch embedding - single call for all chunks
    let engine = crate::embeddings::EmbeddingEngine::new(workspace.to_path_buf());
    let embeddings = engine.embed_chunks(base_name, &all_chunks, None).await?;
    progress.embed(total_chunks as u64, Some(total_chunks as u64), &config.model);

    // Batch insert - collect all KnowledgeChunks first
    let mut knowledge_chunks = Vec::new();
    for (chunk_item, embedding) in all_chunks.into_iter().zip(embeddings) {
        let knowledge_chunk = KnowledgeChunk {
            id: chunk_item.id,
            source_id: chunk_item.source_id,
            position: chunk_item.position,
            text: chunk_item.text,
            embedding: Some(embedding),
            metadata: serde_json::to_value(&chunk_item.metadata)?,
        };
        knowledge_chunks.push(knowledge_chunk);
    }

    // Batch upsert
    index.upsert_chunks(&knowledge_chunks)?;
    progress.index(total_chunks as u64, Some(total_chunks as u64));

    // Track sources
    let mut sources_count = 0u32;
    let mut chunks_count = 0u32;
    let mut bytes_processed = 0u64;
    
    for (source_id, chunks, path, byte_count) in pending.drain(..) {
        let source = KnowledgeSource {
            source_id,
            path: path.to_string_lossy().to_string(),
            source_type: "file".to_string(),
            indexed_at: chrono::Utc::now(),
            chunk_count: chunks.len() as u32,
            byte_count,
        };
        source_manager.track_source(&source)?;
        
        sources_count += 1;
        chunks_count += chunks.len() as u32;
        bytes_processed += byte_count;
    }

    Ok((sources_count, chunks_count, bytes_processed))
}

/// Check if a file should be included based on patterns.
fn should_include(path: &Path, options: &LearnOptions) -> bool {
    let path_str = path.to_string_lossy();

    // Default exclusions (always applied)
    const DEFAULT_EXCLUDES: &[&str] = &[
        "/.git/",
        "/.svn/",
        "/.hg/",
        "/node_modules/",
        "/.next/",
        "/dist/",
        "/build/",
        "/target/",
        "/.venv/",
        "/__pycache__/",
        "/.pytest_cache/",
        "/.mypy_cache/",
        "/vendor/",
        "/.idea/",
        "/.vscode/",
        "/.DS_Store",
        ".min.js",
        ".min.css",
        ".map",
        ".lock",
        ".log",
        ".tmp",
        ".temp",
        ".cache",
    ];

    // Check default exclusions
    for pattern in DEFAULT_EXCLUDES {
        if path_str.contains(pattern) {
            tracing::debug!("Excluding file (default pattern '{}'): {:?}", pattern, path);
            return false;
        }
    }

    // Check user-provided excludes
    for pattern in &options.exclude {
        if path_str.contains(pattern) {
            tracing::debug!("Excluding file (user pattern '{}'): {:?}", pattern, path);
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
        tracing::debug!("Excluding file (no include match): {:?}", path);
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
