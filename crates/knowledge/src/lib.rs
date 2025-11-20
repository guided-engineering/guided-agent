//! Knowledge base management system.
//!
//! Provides local-first RAG using SQLite and embeddings.

pub mod chunker;
pub mod config;
pub mod index;
pub mod parser;
pub mod types;

// Re-export commonly used types
pub use types::{AskOptions, AskResult, BaseStats, KnowledgeBaseConfig, KnowledgeChunk, KnowledgeSource, LearnOptions, LearnStats};

use chrono::Utc;
use guided_core::{AppError, AppResult};
use guided_llm::{create_client, LlmClient};
use std::path::Path;
use std::time::Instant;
use types::*;
use walkdir::WalkDir;

/// Learn from sources and populate the knowledge base.
pub async fn learn(
    workspace: &Path,
    options: LearnOptions,
    api_key: Option<&str>,
) -> AppResult<LearnStats> {
    let start = Instant::now();

    tracing::info!("Starting learn operation for base '{}'", options.base_name);

    // Load or create config
    let config = config::load_config(workspace, &options.base_name)?;

    // Reset if requested
    let index_path = config::get_index_path(workspace, &options.base_name);
    let conn = index::init_index(&index_path)?;

    if options.reset {
        tracing::info!("Resetting knowledge base");
        index::reset_index(&conn)?;
    }

    // Create LLM client for embeddings
    let client = create_client(&config.provider, None, api_key)
        .map_err(|e| AppError::Knowledge(format!("Failed to create embedding client: {}", e)))?;

    let mut sources_count = 0u32;
    let mut chunks_count = 0u32;
    let mut bytes_processed = 0u64;

    // Process paths
    for path in &options.paths {
        if path.is_file() {
            if let Ok(stats) =
                process_file(&conn, client.as_ref(), &config, path, &options).await
            {
                sources_count += 1;
                chunks_count += stats.0;
                bytes_processed += stats.1;
            }
        } else if path.is_dir() {
            for entry in WalkDir::new(path)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let entry_path = entry.path();
                if entry_path.is_file() && should_include(entry_path, &options) {
                    if let Ok(stats) =
                        process_file(&conn, client.as_ref(), &config, entry_path, &options).await
                    {
                        sources_count += 1;
                        chunks_count += stats.0;
                        bytes_processed += stats.1;
                    }
                }
            }
        }
    }

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
async fn process_file(
    conn: &rusqlite::Connection,
    client: &dyn LlmClient,
    config: &KnowledgeBaseConfig,
    path: &Path,
    _options: &LearnOptions,
) -> AppResult<(u32, u64)> {
    tracing::debug!("Processing file: {:?}", path);

    // Parse file
    let text = parser::parse_file(path)?;
    let size_bytes = text.len() as u64;

    // Create source
    let source_id = uuid::Uuid::new_v4().to_string();
    let source = KnowledgeSource {
        id: source_id.clone(),
        path: Some(path.to_path_buf()),
        url: None,
        content_type: parser::ContentType::from_path(path).as_str().to_string(),
        learned_at: Utc::now(),
        size_bytes,
    };

    index::insert_source(conn, &source)?;

    // Chunk text
    let candidates = chunker::chunk_text(
        &source_id,
        &text,
        config.chunk_size as usize,
        config.chunk_overlap as usize,
    );

    let mut chunks_count = 0u32;

    // Embed and insert chunks
    for candidate in candidates {
        let embedding = generate_embedding(client, &config.model, &candidate.text).await?;

        let chunk = KnowledgeChunk {
            id: uuid::Uuid::new_v4().to_string(),
            source_id: candidate.source_id,
            position: candidate.position,
            text: candidate.text,
            embedding: Some(embedding),
            metadata: candidate.metadata,
        };

        index::insert_chunk(conn, &chunk)?;
        chunks_count += 1;
    }

    tracing::debug!(
        "Processed {:?}: {} chunks, {} bytes",
        path,
        chunks_count,
        size_bytes
    );

    Ok((chunks_count, size_bytes))
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

    let conn = index::init_index(&index_path)?;

    // Create LLM client for embeddings
    let client = create_client(&config.provider, None, api_key)
        .map_err(|e| AppError::Knowledge(format!("Failed to create embedding client: {}", e)))?;

    // Generate query embedding
    let query_embedding = generate_embedding(client.as_ref(), &config.model, &options.query).await?;

    // Retrieve top-k chunks
    let results = index::query_chunks(&conn, &query_embedding, options.top_k as usize)?;

    let chunks: Vec<KnowledgeChunk> = results.iter().map(|(chunk, _score)| chunk.clone()).collect();
    let scores: Vec<f32> = results.iter().map(|(_chunk, score)| *score).collect();

    tracing::info!("Retrieved {} chunks", chunks.len());

    Ok(AskResult { chunks, scores })
}

/// Clean (reset) a knowledge base.
pub fn clean(workspace: &Path, base_name: &str) -> AppResult<()> {
    tracing::info!("Cleaning knowledge base '{}'", base_name);

    let index_path = config::get_index_path(workspace, base_name);
    if !index_path.exists() {
        return Err(AppError::Knowledge(format!(
            "Knowledge base '{}' does not exist",
            base_name
        )));
    }

    let conn = index::init_index(&index_path)?;
    index::reset_index(&conn)?;

    tracing::info!("Knowledge base '{}' cleaned", base_name);
    Ok(())
}

/// Get statistics for a knowledge base.
pub fn stats(workspace: &Path, base_name: &str) -> AppResult<BaseStats> {
    tracing::info!("Getting stats for knowledge base '{}'", base_name);

    let index_path = config::get_index_path(workspace, base_name);
    if !index_path.exists() {
        return Err(AppError::Knowledge(format!(
            "Knowledge base '{}' does not exist",
            base_name
        )));
    }

    let conn = index::init_index(&index_path)?;
    let (sources_count, chunks_count) = index::get_stats(&conn)?;

    let db_size_bytes = std::fs::metadata(&index_path)
        .map(|m| m.len())
        .unwrap_or(0);

    Ok(BaseStats {
        base_name: base_name.to_string(),
        sources_count,
        chunks_count,
        db_size_bytes,
        last_learn_at: None, // TODO: Track this in stats.json
    })
}

/// Generate embedding for text using the LLM client.
async fn generate_embedding(
    client: &dyn LlmClient,
    model: &str,
    text: &str,
) -> AppResult<Vec<f32>> {
    // Use the completion endpoint to generate embeddings
    // NOTE: This is a simplified approach. Real embedding models might need a different endpoint.
    // For now, we'll use a deterministic mock based on text hash for testing purposes.
    
    // For production, you'd call an actual embedding endpoint like:
    // client.embed(model, text).await
    
    // Mock implementation using text hash (replace with real embeddings in production)
    let hash = text
        .bytes()
        .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
    
    let dim = 384; // Common embedding dimension
    let mut embedding = Vec::with_capacity(dim);
    
    for i in 0..dim {
        let value = ((hash.wrapping_add(i as u64)) as f32 / u64::MAX as f32) * 2.0 - 1.0;
        embedding.push(value);
    }
    
    // Normalize
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in &mut embedding {
            *v /= norm;
        }
    }
    
    Ok(embedding)
}
