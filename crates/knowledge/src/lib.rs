//! Knowledge base management system.
//!
//! Provides local-first RAG using SQLite and embeddings.

pub mod chunker;
pub mod config;
pub mod index;
pub mod parser;
pub mod types;

#[cfg(test)]
mod tests;

// Re-export commonly used types
pub use types::{
    AskOptions, AskResult, BaseStats, KnowledgeBaseConfig, KnowledgeChunk, KnowledgeSource,
    LearnOptions, LearnStats,
};

use chrono::Utc;
use guided_core::{AppError, AppResult};
use guided_llm::{create_client, LlmClient};
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
            if let Ok(stats) = process_file(&conn, client.as_ref(), &config, path, &options).await {
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
    let query_embedding =
        generate_embedding(client.as_ref(), &config.model, &options.query).await?;

    // Retrieve top-k chunks
    let results = index::query_chunks(&conn, &query_embedding, options.top_k as usize)?;

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

    let db_size_bytes = std::fs::metadata(&index_path).map(|m| m.len()).unwrap_or(0);

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
    _client: &dyn LlmClient,
    _model: &str,
    text: &str,
) -> AppResult<Vec<f32>> {
    // NOTE: This is a mock implementation for Phase 5.
    // Production systems should call actual embedding APIs:
    // client.embed(model, text).await

    // Mock implementation: Create embeddings based on word frequencies and text properties
    // This produces more realistic, content-aware embeddings for testing

    let dim = 384; // Common embedding dimension
    let mut embedding = vec![0.0; dim];

    // Use text properties to create content-aware embeddings
    let lower = text.to_lowercase();

    // Filter stop words for better discrimination
    let stop_words: std::collections::HashSet<&str> = [
        "the", "is", "at", "which", "on", "a", "an", "as", "are", "was", "were", "for", "to", "of",
        "in", "and", "or", "but", "with", "by", "from", "this", "that", "be", "have", "has", "had",
        "it", "its", "their", "they", "them",
    ]
    .iter()
    .copied()
    .collect();

    let words: Vec<&str> = lower
        .split_whitespace()
        .filter(|w| !stop_words.contains(w) && w.len() > 2)
        .collect();

    // Build word frequency map
    let mut word_freq = std::collections::HashMap::new();
    for word in &words {
        *word_freq.entry(*word).or_insert(0) += 1;
    }

    // Map each unique word to multiple dimensions based on character trigrams
    // This creates more specific semantic vectors
    for (word, freq) in word_freq.iter() {
        // Use character trigrams for better semantic encoding
        let chars: Vec<char> = word.chars().collect();
        for i in 0..chars.len().saturating_sub(2) {
            let trigram = format!(
                "{}{}{}",
                chars[i],
                chars[i + 1],
                chars.get(i + 2).unwrap_or(&' ')
            );
            let trigram_hash = trigram
                .bytes()
                .fold(0u64, |acc, b| acc.wrapping_mul(37).wrapping_add(b as u64));

            let dim_idx = (trigram_hash as usize) % dim;
            embedding[dim_idx] += (*freq as f32).sqrt(); // sqrt scale for better distribution
        }

        // Also encode whole word
        let word_hash = word
            .bytes()
            .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
        let base_dim = (word_hash as usize) % dim;
        embedding[base_dim] += *freq as f32;
    }

    // Normalize to unit vector
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in &mut embedding {
            *v /= norm;
        }
    }

    Ok(embedding)
}
