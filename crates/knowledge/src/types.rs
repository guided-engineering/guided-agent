//! Knowledge system type definitions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for a knowledge base.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeBaseConfig {
    /// Name of the knowledge base
    pub name: String,

    /// LLM provider for embeddings
    pub provider: String,

    /// Model for embeddings
    pub model: String,

    /// Chunk size in tokens/characters
    #[serde(default = "default_chunk_size")]
    pub chunk_size: u32,

    /// Overlap between chunks
    #[serde(default = "default_chunk_overlap")]
    pub chunk_overlap: u32,

    /// Maximum context tokens for retrieval
    #[serde(default = "default_max_context_tokens")]
    pub max_context_tokens: u32,

    /// Embedding vector dimension
    #[serde(default = "default_embedding_dim")]
    pub embedding_dim: u32,
}

fn default_chunk_size() -> u32 {
    512
}

fn default_chunk_overlap() -> u32 {
    64
}

fn default_embedding_dim() -> u32 {
    384
}

fn default_max_context_tokens() -> u32 {
    2048
}

impl Default for KnowledgeBaseConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            provider: "ollama".to_string(),
            model: "nomic-embed-text".to_string(),
            chunk_size: default_chunk_size(),
            chunk_overlap: default_chunk_overlap(),
            max_context_tokens: default_max_context_tokens(),
            embedding_dim: default_embedding_dim(),
        }
    }
}

/// Represents a source document in the knowledge base (sources.jsonl tracking).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeSource {
    /// Unique source identifier
    pub source_id: String,

    /// Source path (file path or URL)
    pub path: String,

    /// Source type: "file", "url", "zip"
    pub source_type: String,

    /// When this source was indexed
    pub indexed_at: DateTime<Utc>,

    /// Number of chunks created from this source
    pub chunk_count: u32,

    /// Source size in bytes
    pub byte_count: u64,
}

/// A text chunk with embedding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeChunk {
    /// Unique chunk identifier
    pub id: String,

    /// Source document ID
    pub source_id: String,

    /// Position within source
    pub position: u32,

    /// Text content
    pub text: String,

    /// Embedding vector (normalized)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,

    /// Metadata (e.g., file path, line numbers)
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// Options for the learn operation.
#[derive(Debug, Clone)]
pub struct LearnOptions {
    /// Knowledge base name
    pub base_name: String,

    /// Local paths to learn from
    pub paths: Vec<PathBuf>,

    /// URLs to fetch and learn
    pub urls: Vec<String>,

    /// Include patterns (glob)
    pub include: Vec<String>,

    /// Exclude patterns (glob)
    pub exclude: Vec<String>,

    /// Reset the base before learning
    pub reset: bool,
}

/// Statistics from a learn operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnStats {
    /// Number of sources processed
    pub sources_count: u32,

    /// Number of chunks created
    pub chunks_count: u32,

    /// Total bytes processed
    pub bytes_processed: u64,

    /// Duration in seconds
    pub duration_secs: f64,
}

/// Options for the ask operation.
#[derive(Debug, Clone)]
pub struct AskOptions {
    /// Knowledge base name
    pub base_name: String,

    /// Query text
    pub query: String,

    /// Number of chunks to retrieve
    pub top_k: u32,
}

/// Result from a knowledge retrieval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskResult {
    /// Retrieved chunks (sorted by relevance)
    pub chunks: Vec<KnowledgeChunk>,

    /// Relevance scores
    pub scores: Vec<f32>,
}

/// Statistics for a knowledge base.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseStats {
    /// Base name
    pub base_name: String,

    /// Number of sources
    pub sources_count: u32,

    /// Number of chunks
    pub chunks_count: u32,

    /// Database size in bytes
    pub db_size_bytes: u64,

    /// Last learn timestamp
    pub last_learn_at: Option<DateTime<Utc>>,
}

/// Internal chunk candidate before embedding.
#[derive(Debug, Clone)]
pub struct ChunkCandidate {
    pub source_id: String,
    pub position: u32,
    pub text: String,
    pub metadata: serde_json::Value,
}
