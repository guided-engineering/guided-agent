//! Knowledge base management system.
//!
//! Provides local-first RAG using LanceDB vector index.

pub mod chunk;
pub mod chunker; // Deprecated: use chunk module instead
pub mod config;
pub mod lancedb_index;
pub mod parser;
pub mod types;
pub mod vector_index;

#[cfg(test)]
mod tests;

// Re-export commonly used types
pub use types::{
    AskOptions, AskResult, BaseStats, KnowledgeBaseConfig, KnowledgeChunk, KnowledgeSource,
    LearnOptions, LearnStats,
};

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

    // Initialize LanceDB index
    let index_path = config::get_index_path(workspace, &options.base_name);
    let mut index =
        lancedb_index::LanceDbIndex::new(&index_path, "chunks", config.embedding_dim as usize)
            .await?;

    // Reset if requested
    if options.reset {
        tracing::info!("Resetting knowledge base");
        use vector_index::VectorIndex;
        index.reset()?;
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
                process_file(&mut index, client.as_ref(), &config, path, &options).await
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
                        process_file(&mut index, client.as_ref(), &config, entry_path, &options)
                            .await
                    {
                        sources_count += 1;
                        chunks_count += stats.0;
                        bytes_processed += stats.1;
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
async fn process_file(
    index: &mut dyn vector_index::VectorIndex,
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
    let chunks = pipeline.process(&source_id, &text, Some(path))?;

    let mut chunks_count = 0u32;

    // Embed and insert chunks
    for chunk_item in chunks {
        let embedding = generate_embedding(client, &config.model, &chunk_item.text).await?;

        let knowledge_chunk = KnowledgeChunk {
            id: chunk_item.id,
            source_id: chunk_item.source_id,
            position: chunk_item.position,
            text: chunk_item.text,
            embedding: Some(embedding),
            metadata: serde_json::to_value(&chunk_item.metadata)?,
        };

        index.upsert_chunk(&knowledge_chunk)?;
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

    // Initialize LanceDB index
    let index =
        lancedb_index::LanceDbIndex::new(&index_path, "chunks", config.embedding_dim as usize)
            .await?;

    // Create LLM client for embeddings
    let client = create_client(&config.provider, None, api_key)
        .map_err(|e| AppError::Knowledge(format!("Failed to create embedding client: {}", e)))?;

    // Generate query embedding
    let query_embedding =
        generate_embedding(client.as_ref(), &config.model, &options.query).await?;

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

    tracing::info!("Knowledge base '{}' cleaned", base_name);
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
