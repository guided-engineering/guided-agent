//! RAG (Retrieval-Augmented Generation) answering system.
//!
//! Provides natural language answering over knowledge bases using LLM synthesis.

pub mod ask;
pub mod types;

pub use types::{RagResponse, RagSourceRef};
