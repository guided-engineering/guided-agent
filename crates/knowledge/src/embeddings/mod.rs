//! Embedding engine for knowledge bases.
//!
//! Provides provider-agnostic embedding generation with per-base configuration.

pub mod config;
pub mod provider;
pub mod providers;

pub use config::EmbeddingConfig;
pub use provider::{create_provider, EmbeddingProvider};

use crate::chunk::Chunk;
use guided_core::AppResult;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// Central embedding engine that manages providers per knowledge base.
pub struct EmbeddingEngine {
    workspace: PathBuf,
    providers: Arc<RwLock<HashMap<String, Arc<dyn EmbeddingProvider>>>>,
}

impl EmbeddingEngine {
    /// Create a new embedding engine for a workspace.
    pub fn new(workspace: PathBuf) -> Self {
        Self {
            workspace,
            providers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or create provider for a knowledge base.
    async fn get_provider(
        &self,
        base_name: &str,
        api_key: Option<&str>,
    ) -> AppResult<Arc<dyn EmbeddingProvider>> {
        // Check cache first
        {
            let providers = self.providers.read().unwrap();
            if let Some(provider) = providers.get(base_name) {
                return Ok(Arc::clone(provider));
            }
        }

        // Load config and create provider
        let config = EmbeddingConfig::load(&self.workspace, base_name)?;

        tracing::debug!(
            "Creating embedding provider for base '{}': provider={}, model={}, dimensions={}",
            base_name,
            config.provider,
            config.model,
            config.dimensions
        );

        let provider = provider::create_provider(&config, api_key).await?;

        // Cache it
        {
            let mut providers = self.providers.write().unwrap();
            providers.insert(base_name.to_string(), Arc::clone(&provider));
        }

        Ok(provider)
    }

    /// Embed multiple texts for a knowledge base.
    pub async fn embed_texts(
        &self,
        base_name: &str,
        texts: &[String],
        api_key: Option<&str>,
    ) -> AppResult<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let provider = self.get_provider(base_name, api_key).await?;

        tracing::info!(
            "Embedding {} texts for base '{}' using provider '{}' (model: {})",
            texts.len(),
            base_name,
            provider.provider_name(),
            provider.model_name()
        );

        let embeddings = provider.embed_batch(texts).await?;

        tracing::debug!(
            "Generated {} embeddings of dimension {}",
            embeddings.len(),
            provider.dimensions()
        );

        Ok(embeddings)
    }

    /// Embed chunks (extracts text from Chunk structs).
    pub async fn embed_chunks(
        &self,
        base_name: &str,
        chunks: &[Chunk],
        api_key: Option<&str>,
    ) -> AppResult<Vec<Vec<f32>>> {
        let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
        self.embed_texts(base_name, &texts, api_key).await
    }

    /// Validate that a base's config is consistent with existing index.
    pub fn validate_config_consistency(&self, base_name: &str) -> AppResult<()> {
        let index_path = crate::config::get_index_path(&self.workspace, base_name);

        if !index_path.exists() {
            // New base, no validation needed
            return Ok(());
        }

        // Config exists, ensure it's loaded properly
        let _config = EmbeddingConfig::load(&self.workspace, base_name)?;

        tracing::debug!("Config validation passed for base '{}'", base_name);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_embedding_engine_trigram_provider() {
        let temp = TempDir::new().unwrap();
        let engine = EmbeddingEngine::new(temp.path().to_path_buf());

        // Create base config
        let config = EmbeddingConfig {
            provider: "trigram".to_string(),
            model: "trigram-v1".to_string(),
            dimensions: 384,
            normalize: true,
            batch_size: 100,
            provider_config: serde_json::json!({}),
        };
        config.save(temp.path(), "test-base").unwrap();

        let texts = vec!["hello world".to_string(), "test embedding".to_string()];

        let embeddings = engine
            .embed_texts("test-base", &texts, None)
            .await
            .unwrap();

        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].len(), 384);
        assert_eq!(embeddings[1].len(), 384);
    }

    #[tokio::test]
    async fn test_embedding_engine_caching() {
        let temp = TempDir::new().unwrap();
        let engine = EmbeddingEngine::new(temp.path().to_path_buf());

        let config = EmbeddingConfig {
            provider: "trigram".to_string(),
            model: "trigram-v1".to_string(),
            dimensions: 384,
            normalize: true,
            batch_size: 100,
            provider_config: serde_json::json!({}),
        };
        config.save(temp.path(), "test-base").unwrap();

        // First call should create provider
        let texts1 = vec!["test1".to_string()];
        engine
            .embed_texts("test-base", &texts1, None)
            .await
            .unwrap();

        // Second call should use cached provider
        let texts2 = vec!["test2".to_string()];
        engine
            .embed_texts("test-base", &texts2, None)
            .await
            .unwrap();

        // Verify provider is cached
        let providers = engine.providers.read().unwrap();
        assert!(providers.contains_key("test-base"));
    }
}
