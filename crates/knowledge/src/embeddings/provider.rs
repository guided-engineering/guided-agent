//! Embedding provider trait and factory.

use crate::embeddings::config::EmbeddingConfig;
use guided_core::{AppError, AppResult};
use std::sync::Arc;

/// Trait for embedding providers.
#[async_trait::async_trait]
pub trait EmbeddingProvider: Send + Sync + std::fmt::Debug {
    /// Get provider name (e.g., "mock", "openai", "ollama")
    fn provider_name(&self) -> &str;

    /// Get model identifier
    fn model_name(&self) -> &str;

    /// Get embedding dimensions
    fn dimensions(&self) -> usize;

    /// Generate embeddings for multiple texts in a batch.
    async fn embed_batch(&self, texts: &[String]) -> AppResult<Vec<Vec<f32>>>;

    /// Generate embedding for a single text (convenience method).
    async fn embed(&self, text: &str) -> AppResult<Vec<f32>> {
        let mut results = self.embed_batch(&[text.to_string()]).await?;
        results
            .pop()
            .ok_or_else(|| AppError::Knowledge("No embedding returned".to_string()))
    }
}

/// Create an embedding provider based on configuration.
pub fn create_provider(
    config: &EmbeddingConfig,
    _api_key: Option<&str>,
) -> AppResult<Arc<dyn EmbeddingProvider>> {
    match config.provider.as_str() {
        "mock" => {
            let provider = super::providers::mock::MockProvider::new(config.dimensions);
            Ok(Arc::new(provider))
        }

        "openai" => Err(AppError::Knowledge(
            "OpenAI provider not yet implemented. Use 'mock' provider for now.".to_string(),
        )),

        "ollama" => Err(AppError::Knowledge(
            "Ollama provider not yet implemented. Use 'mock' provider for now.".to_string(),
        )),

        "gguf" => Err(AppError::Knowledge(
            "GGUF provider not yet implemented. Use 'mock' provider for now.".to_string(),
        )),

        _ => Err(AppError::Knowledge(format!(
            "Unknown embedding provider: '{}'. Supported providers: mock, openai, ollama, gguf",
            config.provider
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_mock_provider() {
        let config = EmbeddingConfig {
            provider: "mock".to_string(),
            model: "trigram-v1".to_string(),
            dimensions: 384,
            normalize: true,
            batch_size: 100,
            provider_config: serde_json::json!({}),
        };

        let provider = create_provider(&config, None).unwrap();
        assert_eq!(provider.provider_name(), "mock");
        assert_eq!(provider.model_name(), "trigram-v1");
        assert_eq!(provider.dimensions(), 384);
    }

    #[test]
    fn test_create_unknown_provider() {
        let config = EmbeddingConfig {
            provider: "unknown".to_string(),
            model: "test".to_string(),
            dimensions: 384,
            normalize: true,
            batch_size: 100,
            provider_config: serde_json::json!({}),
        };

        let result = create_provider(&config, None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unknown embedding provider"));
    }

    #[tokio::test]
    async fn test_provider_embed_single() {
        let config = EmbeddingConfig::default();
        let provider = create_provider(&config, None).unwrap();

        let embedding = provider.embed("test text").await.unwrap();
        assert_eq!(embedding.len(), 384);
    }
}
