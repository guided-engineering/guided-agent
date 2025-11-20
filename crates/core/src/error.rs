//! Error types for the Guided Agent CLI.
//!
//! This module defines a unified error enum that covers all error categories
//! in the application, including configuration, I/O, LLM, knowledge, prompt,
//! and task errors.

use thiserror::Error;

/// Unified error type for the Guided Agent CLI.
///
/// All functions in the application return `Result<T, AppError>`.
/// We never panic — errors must be represented and propagated.
#[derive(Error, Debug)]
pub enum AppError {
    /// Configuration-related errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// I/O and filesystem errors
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// LLM provider errors
    #[error("LLM error: {0}")]
    Llm(String),

    /// Knowledge base and RAG errors
    #[error("Knowledge error: {0}")]
    Knowledge(String),

    /// Prompt system errors
    #[error("Prompt error: {0}")]
    Prompt(String),

    /// Task planning and execution errors
    #[error("Task error: {0}")]
    Task(String),

    /// Serialization/deserialization errors
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Generic errors
    #[error("{0}")]
    Other(String),
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::Serialization(err.to_string())
    }
}

impl From<serde_yaml::Error> for AppError {
    fn from(err: serde_yaml::Error) -> Self {
        AppError::Serialization(err.to_string())
    }
}

/// Convenience type alias for Results with AppError.
pub type AppResult<T> = Result<T, AppError>;
