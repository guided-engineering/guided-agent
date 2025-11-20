//! Hybrid chunking pipeline for semantic text and code splitting.
//!
//! This module provides intelligent chunking that:
//! - Detects content type (text, markdown, code, HTML)
//! - Uses appropriate splitter (text-splitter, tree-sitter, fallback)
//! - Preserves semantic boundaries
//! - Generates rich metadata

mod detection;
mod merging;
mod metadata;
mod pipeline;
pub mod splitters;

pub use detection::{ContentType, Language};
pub use pipeline::{ChunkConfig, ChunkPipeline};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A semantic chunk with rich metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    /// Unique identifier (UUID v4)
    pub id: String,
    
    /// Source file/document identifier
    pub source_id: String,
    
    /// Chunk position in document (0-indexed)
    pub position: u32,
    
    /// Chunk text content
    pub text: String,
    
    /// Rich metadata about the chunk
    pub metadata: ChunkMetadata,
}

/// Metadata about a chunk's origin and characteristics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMetadata {
    /// Content type detected
    pub content_type: ContentType,
    
    /// Programming language (if code)
    pub language: Option<Language>,
    
    /// Byte range in original document
    pub byte_range: (usize, usize),
    
    /// Line range in original document (if available)
    pub line_range: Option<(usize, usize)>,
    
    /// Character count
    pub char_count: usize,
    
    /// Token count (if tokenizer available)
    pub token_count: Option<usize>,
    
    /// SHA-256 hash of chunk text
    pub hash: String,
    
    /// Timestamp when chunk was created
    pub created_at: DateTime<Utc>,
    
    /// Splitter used ("text-splitter" | "code-splitter" | "fallback")
    pub splitter_used: String,
    
    /// Custom metadata (extensible)
    #[serde(default)]
    pub custom: serde_json::Value,
}

impl Chunk {
    /// Create a new chunk with generated ID and timestamp.
    pub fn new(
        source_id: String,
        position: u32,
        text: String,
        byte_range: (usize, usize),
        content_type: ContentType,
        splitter_used: String,
    ) -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        let char_count = text.chars().count();
        let hash = metadata::calculate_hash(&text);
        
        Self {
            id,
            source_id,
            position,
            text,
            metadata: ChunkMetadata {
                content_type,
                language: None,
                byte_range,
                line_range: None,
                char_count,
                token_count: None,
                hash,
                created_at: Utc::now(),
                splitter_used,
                custom: serde_json::json!({}),
            },
        }
    }
}
