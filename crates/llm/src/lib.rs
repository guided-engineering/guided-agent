//! LLM integration crate for Guided Agent CLI.
//!
//! This crate provides a provider-agnostic abstraction for interacting with
//! Large Language Models (LLMs). It supports multiple providers through a
//! unified trait-based interface.
//!
//! # Providers
//! - **Ollama**: Local LLM runtime (default)
//! - Future: OpenAI, Anthropic, etc.
//!
//! # Example
//! ```no_run
//! use guided_llm::{LlmClient, LlmRequest, providers::OllamaClient};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = OllamaClient::new();
//! let request = LlmRequest::new("Hello, world!", "llama3");
//! let response = client.complete(&request).await?;
//! println!("{}", response.content);
//! # Ok(())
//! # }
//! ```

pub mod client;
pub mod factory;
pub mod providers;
pub mod types;

// Re-export main types
pub use client::{LlmClient, LlmRequest, LlmResponse, LlmStream, LlmStreamChunk, LlmUsage};
pub use factory::create_client;
pub use providers::OllamaClient;
pub use types::{LlmConfig, LlmProviderConfig, ProviderType};
