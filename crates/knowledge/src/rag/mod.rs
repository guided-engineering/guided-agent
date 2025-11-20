//! RAG (Retrieval-Augmented Generation) answering system.
//!
//! Provides natural language answering over knowledge bases using LLM synthesis.

pub mod ask;
pub mod search;
pub mod sources;
pub mod types;

pub use search::{detect_query_filters, SearchFilters};
pub use sources::SourceManager;
pub use types::{RagResponse, RagSourceRef};
