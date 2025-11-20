//! Prompt system for the Guided Agent CLI.
//!
//! This crate provides structured prompt management with:
//! - YAML-based prompt definitions
//! - Handlebars template rendering
//! - Workspace context injection
//! - Knowledge base context injection

pub mod builder;
pub mod loader;
pub mod types;

// Re-export main types
pub use builder::build_prompt;
pub use loader::{list_prompts, load_prompt};
pub use types::{
    BuiltPrompt, BuiltPromptMetadata, PromptBehavior, PromptContextConfig, PromptDefinition,
    PromptInputSpec, PromptOutputSpec,
};
