//! Guided Agent Core Library
//!
//! This crate provides the foundational utilities for the Guided Agent CLI:
//! - Error handling (`AppError`, `AppResult`)
//! - Logging infrastructure
//! - Configuration management
//! - Shared types and helpers

pub mod config;
pub mod error;
pub mod logging;

// Re-export commonly used types
pub use config::AppConfig;
pub use error::{AppError, AppResult};
