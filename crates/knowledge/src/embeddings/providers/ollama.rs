//! Ollama Embedding Provider
//!
//! Provides semantic embeddings via Ollama's local API using models like nomic-embed-text.
//!
//! # Features
//! - Neural semantic embeddings (768-dim by default)
//! - Local-first (no API costs, privacy-preserving)
//! - Multilingual support (100+ languages)
//! - Batch embedding support
//! - Automatic retry with exponential backoff
//!
//! # Example
//! ```no_run
//! use guided_agent_knowledge::embeddings::{EmbeddingConfig, EmbeddingProvider};
//! use guided_agent_knowledge::embeddings::providers::ollama::OllamaProvider;
//!
//! # tokio_test::block_on(async {
//! let config = EmbeddingConfig {
//!     provider: "ollama".to_string(),
//!     model: "nomic-embed-text".to_string(),
//!     dimensions: 768,
//!     ..Default::default()
//! };
//!
//! let provider = OllamaProvider::new(config).await.unwrap();
//! let embedding = provider.embed("Hello world").await.unwrap();
//! assert_eq!(embedding.len(), 768);
//! # });
//! ```

use crate::embeddings::EmbeddingProvider;
use crate::embeddings::EmbeddingConfig;
use crate::AppError;
use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, instrument, warn};

/// Ollama API endpoint for embeddings
const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";
const EMBEDDING_ENDPOINT: &str = "/api/embeddings";

/// Maximum retry attempts for failed requests
const MAX_RETRIES: u32 = 3;

/// Initial backoff duration in milliseconds
const INITIAL_BACKOFF_MS: u64 = 100;

/// Request timeout in seconds
const REQUEST_TIMEOUT_SECS: u64 = 30;

/// Ollama embedding provider using local API
#[derive(Debug, Clone)]
pub struct OllamaProvider {
    /// HTTP client for API requests
    client: Arc<Client>,
    /// Ollama API base URL
    base_url: String,
    /// Model name (e.g., "nomic-embed-text")
    model: String,
    /// Expected embedding dimensions
    dimensions: usize,
}

/// Request payload for Ollama embeddings API
#[derive(Debug, Clone, Serialize)]
struct EmbeddingRequest {
    /// Model name to use
    model: String,
    /// Text to embed
    prompt: String,
}

/// Response from Ollama embeddings API
#[derive(Debug, Clone, Deserialize)]
struct EmbeddingResponse {
    /// Embedding vector
    embedding: Vec<f32>,
}

/// Error response from Ollama API
#[derive(Debug, Clone, Deserialize)]
struct ErrorResponse {
    /// Error message
    error: String,
}

impl OllamaProvider {
    /// Create new Ollama provider with configuration
    ///
    /// # Arguments
    /// * `config` - Embedding configuration
    ///
    /// # Returns
    /// * `Result<Self, AppError>` - Provider instance or error
    ///
    /// # Errors
    /// * `AppError::LLM` - If Ollama is not reachable or model is invalid
    pub async fn new(config: EmbeddingConfig) -> Result<Self, AppError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .map_err(|e| {
                AppError::Llm(format!("Failed to create HTTP client for Ollama: {}", e))
            })?;

        let base_url = std::env::var("OLLAMA_URL").unwrap_or_else(|_| DEFAULT_OLLAMA_URL.to_string());

        let provider = Self {
            client: Arc::new(client),
            base_url,
            model: config.model.clone(),
            dimensions: config.dimensions,
        };

        // Verify Ollama is running and model is available
        provider.verify_connection().await?;

        Ok(provider)
    }

    /// Verify Ollama connection and model availability
    #[instrument(skip(self), fields(model = %self.model))]
    async fn verify_connection(&self) -> Result<(), AppError> {
        debug!("Verifying Ollama connection at {}", self.base_url);

        // Test with a simple embedding request
        let test_text = "test connection";
        match self.embed_with_retries(test_text, MAX_RETRIES).await {
            Ok(embedding) => {
                if embedding.len() != self.dimensions {
                    return Err(AppError::Llm(format!(
                        "Ollama model '{}' returned {} dimensions, expected {}",
                        self.model,
                        embedding.len(),
                        self.dimensions
                    )));
                }
                debug!("Ollama connection verified, model '{}' ready", self.model);
                Ok(())
            }
            Err(e) => {
                error!("Failed to connect to Ollama: {}", e);
                Err(AppError::Llm(format!(
                    "Ollama not available at {}. Ensure Ollama is running and model '{}' is installed. Run: ollama pull {}",
                    self.base_url, self.model, self.model
                )))
            }
        }
    }

    /// Embed single text with retry logic
    #[instrument(skip(self, text), fields(text_len = text.len(), model = %self.model))]
    async fn embed_with_retries(&self, text: &str, retries: u32) -> Result<Vec<f32>, AppError> {
        let mut attempt = 0;
        let mut last_error = None;

        while attempt < retries {
            match self.embed_single(text).await {
                Ok(embedding) => return Ok(embedding),
                Err(e) => {
                    attempt += 1;
                    last_error = Some(e);

                    if attempt < retries {
                        let backoff_ms = INITIAL_BACKOFF_MS * 2_u64.pow(attempt);
                        warn!(
                            "Embedding failed (attempt {}/{}), retrying in {}ms",
                            attempt, retries, backoff_ms
                        );
                        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| AppError::Llm("Unknown embedding error".to_string())))
    }

    /// Embed single text (no retries)
    #[instrument(skip(self, text), fields(text_len = text.len()))]
    async fn embed_single(&self, text: &str) -> Result<Vec<f32>, AppError> {
        let url = format!("{}{}", self.base_url, EMBEDDING_ENDPOINT);

        let request = EmbeddingRequest {
            model: self.model.clone(),
            prompt: text.to_string(),
        };

        debug!("Sending embedding request to {}", url);

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| AppError::Llm(format!("Failed to send request to Ollama: {}", e)))?;

        let status = response.status();

        if !status.is_success() {
            // Try to parse error response
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            if let Ok(error_response) = serde_json::from_str::<ErrorResponse>(&error_text) {
                return Err(AppError::Llm(format!(
                    "Ollama API error ({}): {}",
                    status, error_response.error
                )));
            }

            return Err(AppError::Llm(format!(
                "Ollama API error ({}): {}",
                status, error_text
            )));
        }

        let response_body: EmbeddingResponse = response
            .json()
            .await
            .map_err(|e| AppError::Llm(format!("Failed to parse Ollama response: {}", e)))?;

        if response_body.embedding.len() != self.dimensions {
            return Err(AppError::Llm(format!(
                "Unexpected embedding dimensions: got {}, expected {}",
                response_body.embedding.len(),
                self.dimensions
            )));
        }

        debug!("Successfully generated {} dimensional embedding", response_body.embedding.len());

        Ok(response_body.embedding)
    }
}

#[async_trait]
impl EmbeddingProvider for OllamaProvider {
    #[instrument(skip(self, text), fields(text_len = text.len(), provider = "ollama", model = %self.model))]
    async fn embed(&self, text: &str) -> Result<Vec<f32>, AppError> {
        if text.trim().is_empty() {
            return Err(AppError::Llm("Cannot embed empty text".to_string()));
        }

        self.embed_with_retries(text, MAX_RETRIES).await
    }

    #[instrument(skip(self, texts), fields(batch_size = texts.len(), provider = "ollama", model = %self.model))]
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, AppError> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        debug!("Embedding batch of {} texts", texts.len());

        // Ollama doesn't support batch API, so we embed sequentially
        // TODO: Consider concurrent requests with semaphore for rate limiting
        let mut embeddings = Vec::with_capacity(texts.len());

        for (i, text) in texts.iter().enumerate() {
            if text.trim().is_empty() {
                warn!("Skipping empty text at index {}", i);
                embeddings.push(vec![0.0; self.dimensions]);
                continue;
            }

            let embedding = self.embed(text).await?;
            embeddings.push(embedding);
        }

        Ok(embeddings)
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn provider_name(&self) -> &str {
        "ollama"
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> EmbeddingConfig {
        EmbeddingConfig {
            provider: "ollama".to_string(),
            model: "nomic-embed-text".to_string(),
            dimensions: 768,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_ollama_provider_creation() {
        // This test requires Ollama to be running locally
        // Skip if OLLAMA_URL is not set
        if std::env::var("OLLAMA_URL").is_err() && !is_ollama_running().await {
            println!("Skipping test: Ollama not running");
            return;
        }

        let config = create_test_config();
        let result = OllamaProvider::new(config).await;
        assert!(result.is_ok(), "Failed to create Ollama provider: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_embed_single() {
        if std::env::var("OLLAMA_URL").is_err() && !is_ollama_running().await {
            println!("Skipping test: Ollama not running");
            return;
        }

        let config = create_test_config();
        let provider = OllamaProvider::new(config).await.unwrap();

        let text = "Hello, world!";
        let embedding = provider.embed(text).await.unwrap();

        assert_eq!(embedding.len(), 768);
        assert!(embedding.iter().any(|&x| x != 0.0), "Embedding should not be all zeros");
    }

    #[tokio::test]
    async fn test_embed_batch() {
        if std::env::var("OLLAMA_URL").is_err() && !is_ollama_running().await {
            println!("Skipping test: Ollama not running");
            return;
        }

        let config = create_test_config();
        let provider = OllamaProvider::new(config).await.unwrap();

        let texts = vec![
            "First text".to_string(),
            "Second text".to_string(),
            "Third text".to_string(),
        ];

        let embeddings = provider.embed_batch(&texts).await.unwrap();

        assert_eq!(embeddings.len(), 3);
        for embedding in embeddings {
            assert_eq!(embedding.len(), 768);
            assert!(embedding.iter().any(|&x| x != 0.0));
        }
    }

    #[tokio::test]
    async fn test_empty_text() {
        if std::env::var("OLLAMA_URL").is_err() && !is_ollama_running().await {
            println!("Skipping test: Ollama not running");
            return;
        }

        let config = create_test_config();
        let provider = OllamaProvider::new(config).await.unwrap();

        let result = provider.embed("").await;
        assert!(result.is_err(), "Should fail on empty text");
    }

    #[tokio::test]
    async fn test_dimensions() {
        if std::env::var("OLLAMA_URL").is_err() && !is_ollama_running().await {
            println!("Skipping test: Ollama not running");
            return;
        }

        let config = create_test_config();
        let provider = OllamaProvider::new(config).await.unwrap();

        assert_eq!(provider.dimensions(), 768);
        assert_eq!(provider.provider_name(), "ollama");
        assert_eq!(provider.model_name(), "nomic-embed-text");
    }

    /// Helper to check if Ollama is running
    async fn is_ollama_running() -> bool {
        let client = Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();

        let url = format!("{}/api/tags", DEFAULT_OLLAMA_URL);
        client.get(&url).send().await.is_ok()
    }
}
