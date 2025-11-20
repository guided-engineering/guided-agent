//! LLM client abstraction and request/response types.
//!
//! This module defines the core abstractions for interacting with LLM providers.

use futures::Stream;
use guided_core::AppResult;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

/// LLM completion request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    /// The prompt text to send to the LLM
    pub prompt: String,

    /// Model identifier (e.g., "llama3", "gpt-4")
    pub model: String,

    /// Maximum tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Temperature for sampling (0.0 - 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Top-p nucleus sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Enable streaming responses
    #[serde(default)]
    pub stream: bool,

    /// System prompt (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
}

impl LlmRequest {
    /// Create a new LLM request with required fields.
    pub fn new(prompt: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            model: model.into(),
            max_tokens: None,
            temperature: None,
            top_p: None,
            stream: false,
            system: None,
        }
    }

    /// Enable streaming for this request.
    pub fn with_streaming(mut self) -> Self {
        self.stream = true;
        self
    }

    /// Set the maximum tokens to generate.
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Set the temperature for sampling.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set the system prompt.
    pub fn with_system(mut self, system: impl Into<String>) -> Self {
        self.system = Some(system.into());
        self
    }
}

/// LLM completion response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    /// The generated text
    pub content: String,

    /// Model that generated the response
    pub model: String,

    /// Usage statistics
    pub usage: LlmUsage,

    /// Whether the response was complete
    #[serde(default = "default_true")]
    pub done: bool,
}

fn default_true() -> bool {
    true
}

/// Token usage statistics.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlmUsage {
    /// Tokens in the prompt
    #[serde(default)]
    pub prompt_tokens: u32,

    /// Tokens in the completion
    #[serde(default)]
    pub completion_tokens: u32,

    /// Total tokens used
    #[serde(default)]
    pub total_tokens: u32,
}

impl LlmUsage {
    /// Create usage stats from prompt and completion token counts.
    pub fn new(prompt_tokens: u32, completion_tokens: u32) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        }
    }
}

/// A chunk from a streaming LLM response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmStreamChunk {
    /// Incremental text content
    pub content: String,

    /// Model generating the stream
    pub model: String,

    /// Whether this is the final chunk
    #[serde(default)]
    pub done: bool,

    /// Usage statistics (only in final chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<LlmUsage>,
}

/// Stream of LLM chunks.
pub type LlmStream = Pin<Box<dyn Stream<Item = AppResult<LlmStreamChunk>> + Send>>;

/// Trait for LLM providers.
///
/// This trait abstracts the underlying LLM provider (Ollama, OpenAI, Anthropic, etc.)
/// and provides a unified interface for completion and streaming.
#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    /// Get the provider name (e.g., "ollama", "openai").
    fn provider_name(&self) -> &str;

    /// Perform a non-streaming completion.
    ///
    /// # Arguments
    /// * `request` - The completion request
    ///
    /// # Returns
    /// The complete LLM response
    async fn complete(&self, request: &LlmRequest) -> AppResult<LlmResponse>;

    /// Perform a streaming completion.
    ///
    /// # Arguments
    /// * `request` - The completion request (will be modified to enable streaming)
    ///
    /// # Returns
    /// A stream of response chunks
    async fn stream(&self, request: &LlmRequest) -> AppResult<LlmStream>;
}
