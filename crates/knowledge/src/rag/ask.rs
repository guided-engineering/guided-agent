//! RAG answering orchestration.
//!
//! Retrieves relevant chunks and generates natural language answers via LLM.

use crate::chunk::ChunkMetadata;
use crate::rag::types::{RagResponse, RagSourceRef, CONFIDENCE_THRESHOLD};
use crate::types::{AskOptions, KnowledgeChunk};
use crate::{config, lancedb_index, vector_index::VectorIndex};
use guided_core::{AppError, AppResult};
use guided_llm::LlmRequest;
use std::collections::HashMap;
use std::path::Path;

/// Minimum cosine similarity score for a chunk to be considered relevant.
const MIN_RELEVANCE_SCORE: f32 = 0.20;

/// Maximum snippet length for source references.
const MAX_SNIPPET_LENGTH: usize = 150;

/// Ask a question and generate a natural language answer using RAG.
///
/// This function:
/// 1. Retrieves relevant chunks from the vector index
/// 2. Checks confidence levels
/// 3. Builds context for the LLM
/// 4. Generates answer via LLM synthesis
/// 5. Maps chunks to human-readable source references
pub async fn ask_rag(
    workspace: &Path,
    options: AskOptions,
    llm_provider: &str,
    api_key: Option<&str>,
) -> AppResult<RagResponse> {
    tracing::info!(
        "RAG answering for knowledge base '{}' with query: {}",
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
    let results = index.search(&query_embedding, options.top_k as usize)?;

    tracing::debug!(
        "Retrieved {} chunks before filtering",
        results.len()
    );

    // Apply relevance cutoff
    let filtered_results: Vec<_> = results
        .into_iter()
        .filter(|(_chunk, score)| *score >= MIN_RELEVANCE_SCORE)
        .collect();

    if filtered_results.is_empty() {
        tracing::info!(
            "No relevant chunks found (all scores below {:.2} threshold)",
            MIN_RELEVANCE_SCORE
        );
        return Ok(RagResponse::no_information(&options.query));
    }

    let chunks: Vec<KnowledgeChunk> = filtered_results
        .iter()
        .map(|(chunk, _score)| chunk.clone())
        .collect();
    let scores: Vec<f32> = filtered_results
        .iter()
        .map(|(_chunk, score)| *score)
        .collect();

    let max_score = scores.first().copied().unwrap_or(0.0);
    let low_confidence = max_score < CONFIDENCE_THRESHOLD;

    tracing::info!(
        "Retrieved {} relevant chunks (max score: {:.3}, low_confidence: {})",
        chunks.len(),
        max_score,
        low_confidence
    );

    // Build context for LLM
    let context = build_context(&chunks)?;

    // Generate answer via LLM
    let answer = generate_answer(
        llm_provider,
        api_key,
        &options.query,
        &context,
        low_confidence,
    )
    .await?;

    // Map chunks to source references
    let sources = map_chunks_to_sources(&chunks);

    Ok(RagResponse::new(answer, sources, max_score))
}

/// Build context string from chunks for LLM prompt.
fn build_context(chunks: &[KnowledgeChunk]) -> AppResult<String> {
    let context_parts: Vec<String> = chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| {
            format!(
                "[Document {}]\n{}",
                i + 1,
                chunk.text
            )
        })
        .collect();

    Ok(context_parts.join("\n\n---\n\n"))
}

/// Generate answer by calling LLM with RAG prompt.
async fn generate_answer(
    provider: &str,
    api_key: Option<&str>,
    query: &str,
    context: &str,
    low_confidence: bool,
) -> AppResult<String> {
    tracing::debug!("Generating answer with LLM (provider: {}, low_confidence: {})", provider, low_confidence);

    // Create LLM client
    let client = guided_llm::create_client(provider, None, api_key)
        .map_err(|e| AppError::Knowledge(format!("Failed to create LLM client: {}", e)))?;

    // Build system prompt
    let system_prompt = build_system_prompt(low_confidence);

    // Build user prompt
    let user_prompt = format!(
        "User question:\n{}\n\nRelevant context from documents:\n{}",
        query, context
    );

    // Create request
    let request = LlmRequest::new(user_prompt, "llama3")
        .with_system(system_prompt)
        .with_temperature(0.3) // Lower temperature for factual answers
        .with_max_tokens(1000);

    // Send request
    let response = client
        .complete(&request)
        .await
        .map_err(|e| AppError::Knowledge(format!("LLM request failed: {}", e)))?;

    Ok(response.content)
}

/// Build system prompt for RAG answering.
fn build_system_prompt(low_confidence: bool) -> String {
    let mut prompt = String::from(
        "You are a knowledge assistant with access to the user's document collection.\n\n"
    );

    if low_confidence {
        prompt.push_str(
            "Note: The retrieved information may not directly answer this question. \
             Be cautious and clear about what the documents do and do not state.\n\n"
        );
    }

    prompt.push_str(
        "Instructions:\n\
         - Provide a clear, direct answer based only on the context provided\n\
         - Do not mention technical terms like \"chunks\", \"embeddings\", \"context\", \"documents\", \"Document 1\", \"Document 2\", etc., or \"RAG\"\n\
         - Do not use phrases like \"Based on the provided information\", \"According to the context\", \"According to Document X\", or \"De acordo com o Documento X\"\n\
         - Answer as if you had read the original documents directly without referring to document numbers\n\
         - Simply state the facts from the documents without saying where they came from\n\
         - If the context suggests but does not confirm something, express that nuance clearly\n\
         - If the context does not contain the answer, state: \"I could not find this information in the available documents.\"\n\
         - Keep your response concise and factual\n"
    );

    prompt
}

/// Map chunks to human-readable source references.
fn map_chunks_to_sources(chunks: &[KnowledgeChunk]) -> Vec<RagSourceRef> {
    // Deduplicate by (source, location)
    let mut seen = HashMap::new();
    let mut sources = Vec::new();

    for chunk in chunks {
        let source = extract_source_name(chunk);
        let location = extract_location(chunk);
        let key = (source.clone(), location.clone());

        if !seen.contains_key(&key) {
            seen.insert(key, true);

            sources.push(RagSourceRef {
                source,
                location,
                snippet: truncate_snippet(&chunk.text, MAX_SNIPPET_LENGTH),
            });
        }
    }

    sources
}

/// Extract human-readable source name from source_id or chunk metadata.
fn extract_source_name(chunk: &KnowledgeChunk) -> String {
    // Try to get source from metadata first
    if let Ok(metadata) = serde_json::from_value::<ChunkMetadata>(chunk.metadata.clone()) {
        if let Some(custom) = metadata.custom.as_object() {
            if let Some(source_path) = custom.get("source_path") {
                if let Some(path_str) = source_path.as_str() {
                    // Extract filename from path
                    if let Some(filename) = path_str.rsplit('/').next() {
                        return filename.to_string();
                    }
                }
            }
        }
    }

    // Fallback: try to parse source_id as path
    if let Some(filename) = chunk.source_id.rsplit('/').next() {
        // Check if it looks like a filename (has extension)
        if filename.contains('.') {
            return filename.to_string();
        }
    }

    // Ultimate fallback: truncate UUID
    if chunk.source_id.len() > 12 {
        format!("{}...", &chunk.source_id[..12])
    } else {
        chunk.source_id.clone()
    }
}

/// Extract human-readable location from chunk metadata.
fn extract_location(chunk: &KnowledgeChunk) -> String {
    // Try to parse metadata
    if let Ok(metadata) = serde_json::from_value::<ChunkMetadata>(chunk.metadata.clone()) {
        if let Some((start, end)) = metadata.line_range {
            return format!("lines {}-{}", start, end);
        }

        // Fallback to byte range
        let (start, end) = metadata.byte_range;
        return format!("byte offset {}-{}", start, end);
    }

    // Ultimate fallback
    format!("position {}", chunk.position)
}

/// Truncate snippet to maximum length.
fn truncate_snippet(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        // Find a good break point (word boundary)
        let truncated = &text[..max_len];
        if let Some(last_space) = truncated.rfind(char::is_whitespace) {
            format!("{}...", &truncated[..last_space])
        } else {
            format!("{}...", truncated)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_source_name() {
        use crate::chunk::ChunkMetadata;
        
        // Test with source_path in metadata
        let mut custom_map = serde_json::Map::new();
        custom_map.insert("source_path".to_string(), serde_json::json!("test-gamedex.md"));
        
        let metadata = ChunkMetadata {
            content_type: crate::chunk::ContentType::Text,
            language: None,
            byte_range: (0, 100),
            line_range: None,
            char_count: 100,
            token_count: None,
            hash: "test".to_string(),
            created_at: chrono::Utc::now(),
            splitter_used: "test".to_string(),
            custom: serde_json::Value::Object(custom_map),
        };
        
        let chunk = KnowledgeChunk {
            id: "1".to_string(),
            source_id: "uuid-12345".to_string(),
            position: 0,
            text: "test".to_string(),
            embedding: None,
            metadata: serde_json::to_value(&metadata).unwrap(),
        };
        
        assert_eq!(extract_source_name(&chunk), "test-gamedex.md");
        
        // Test with UUID fallback
        let chunk_no_path = KnowledgeChunk {
            id: "1".to_string(),
            source_id: "uuid-12345-67890-abcdef".to_string(),
            position: 0,
            text: "test".to_string(),
            embedding: None,
            metadata: serde_json::json!({}),
        };
        
        assert_eq!(extract_source_name(&chunk_no_path), "uuid-12345-6...");
    }

    #[test]
    fn test_truncate_snippet() {
        let short = "Short text";
        assert_eq!(truncate_snippet(short, 100), "Short text");

        let long = "This is a very long text that needs to be truncated at some point";
        let result = truncate_snippet(long, 30);
        assert!(result.len() <= 33); // 30 + "..."
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_build_context() {
        let chunks = vec![
            KnowledgeChunk {
                id: "1".to_string(),
                source_id: "test.md".to_string(),
                position: 0,
                text: "First chunk".to_string(),
                embedding: None,
                metadata: serde_json::json!({}),
            },
            KnowledgeChunk {
                id: "2".to_string(),
                source_id: "test.md".to_string(),
                position: 1,
                text: "Second chunk".to_string(),
                embedding: None,
                metadata: serde_json::json!({}),
            },
        ];

        let context = build_context(&chunks).unwrap();
        assert!(context.contains("First chunk"));
        assert!(context.contains("Second chunk"));
        assert!(context.contains("[Document 1]"));
        assert!(context.contains("[Document 2]"));
        assert!(context.contains("---"));
    }

    #[test]
    fn test_build_system_prompt_normal() {
        let prompt = build_system_prompt(false);
        assert!(prompt.contains("knowledge assistant"));
        assert!(prompt.contains("Do not mention"));
        assert!(!prompt.contains("may not directly answer"));
    }

    #[test]
    fn test_build_system_prompt_low_confidence() {
        let prompt = build_system_prompt(true);
        assert!(prompt.contains("may not directly answer"));
        assert!(prompt.contains("Be cautious"));
    }
}
