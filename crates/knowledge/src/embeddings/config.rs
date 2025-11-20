//! Embedding configuration types and management.

use guided_core::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Embedding configuration for a knowledge base.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmbeddingConfig {
    /// Provider name: "trigram", "openai", "ollama", "gguf"
    pub provider: String,

    /// Model identifier (provider-specific)
    pub model: String,

    /// Embedding vector dimensions
    pub dimensions: usize,

    /// Whether to normalize embeddings to unit length
    #[serde(default = "default_normalize")]
    pub normalize: bool,

    /// Maximum batch size for embedding requests
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    /// Provider-specific configuration (JSON object)
    #[serde(default)]
    pub provider_config: serde_json::Value,
}

fn default_normalize() -> bool {
    true
}

fn default_batch_size() -> usize {
    100
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            provider: "trigram".to_string(),
            model: "trigram-v1".to_string(),
            dimensions: 384,
            normalize: true,
            batch_size: 100,
            provider_config: serde_json::json!({}),
        }
    }
}

impl EmbeddingConfig {
    /// Load embedding config from base config.yaml
    pub fn load(workspace: &Path, base_name: &str) -> AppResult<Self> {
        let config_path = crate::config::get_config_path(workspace, base_name);

        if !config_path.exists() {
            tracing::debug!(
                "No config file for base '{}', using default embedding config",
                base_name
            );
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&config_path).map_err(|e| {
            AppError::Knowledge(format!("Failed to read config at {:?}: {}", config_path, e))
        })?;

        // Parse full KnowledgeBaseConfig and extract embedding settings
        let base_config: crate::types::KnowledgeBaseConfig =
            serde_yaml::from_str(&content).map_err(|e| {
                AppError::Knowledge(format!(
                    "Failed to parse config at {:?}: {}",
                    config_path, e
                ))
            })?;

        // Convert to EmbeddingConfig
        Ok(Self {
            provider: base_config.provider.clone(),
            model: base_config.model.clone(),
            dimensions: base_config.embedding_dim as usize,
            normalize: true,
            batch_size: 100,
            provider_config: serde_json::json!({}),
        })
    }

    /// Save embedding config to base config.yaml
    pub fn save(&self, workspace: &Path, base_name: &str) -> AppResult<()> {
        let config_path = crate::config::get_config_path(workspace, base_name);

        // Ensure directory exists
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                AppError::Knowledge(format!("Failed to create config directory: {}", e))
            })?;
        }

        // Load existing config or create new one
        let mut base_config = if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            serde_yaml::from_str(&content).map_err(|e| {
                AppError::Knowledge(format!("Failed to parse existing config: {}", e))
            })?
        } else {
            crate::types::KnowledgeBaseConfig {
                name: base_name.to_string(),
                ..Default::default()
            }
        };

        // Update embedding-related fields
        base_config.provider = self.provider.clone();
        base_config.model = self.model.clone();
        base_config.embedding_dim = self.dimensions as u32;

        let yaml = serde_yaml::to_string(&base_config)
            .map_err(|e| AppError::Knowledge(format!("Failed to serialize config: {}", e)))?;

        fs::write(&config_path, yaml).map_err(|e| {
            AppError::Knowledge(format!(
                "Failed to write config to {:?}: {}",
                config_path, e
            ))
        })?;

        tracing::debug!("Saved embedding config for base '{}'", base_name);
        Ok(())
    }

    /// Validate that another config is consistent with this one.
    pub fn validate_consistency(&self, other: &Self) -> AppResult<()> {
        if self.provider != other.provider {
            return Err(AppError::Knowledge(format!(
                "Provider mismatch: expected '{}', got '{}'",
                self.provider, other.provider
            )));
        }

        if self.model != other.model {
            return Err(AppError::Knowledge(format!(
                "Model mismatch: expected '{}', got '{}'",
                self.model, other.model
            )));
        }

        if self.dimensions != other.dimensions {
            return Err(AppError::Knowledge(format!(
                "Dimension mismatch: expected {}, got {}",
                self.dimensions, other.dimensions
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = EmbeddingConfig::default();
        assert_eq!(config.provider, "trigram");
        assert_eq!(config.model, "trigram-v1");
        assert_eq!(config.dimensions, 384);
        assert!(config.normalize);
        assert_eq!(config.batch_size, 100);
    }

    #[test]
    fn test_save_and_load() {
        let temp = TempDir::new().unwrap();
        let config = EmbeddingConfig {
            provider: "openai".to_string(),
            model: "text-embedding-3-small".to_string(),
            dimensions: 1536,
            normalize: true,
            batch_size: 100,
            provider_config: serde_json::json!({"api_base": "https://api.openai.com/v1"}),
        };

        config.save(temp.path(), "test-base").unwrap();

        let loaded = EmbeddingConfig::load(temp.path(), "test-base").unwrap();
        assert_eq!(loaded.provider, "openai");
        assert_eq!(loaded.model, "text-embedding-3-small");
        assert_eq!(loaded.dimensions, 1536);
    }

    #[test]
    fn test_validate_consistency_success() {
        let config1 = EmbeddingConfig {
            provider: "trigram".to_string(),
            model: "trigram-v1".to_string(),
            dimensions: 384,
            normalize: true,
            batch_size: 100,
            provider_config: serde_json::json!({}),
        };

        let config2 = config1.clone();
        assert!(config1.validate_consistency(&config2).is_ok());
    }

    #[test]
    fn test_validate_consistency_provider_mismatch() {
        let config1 = EmbeddingConfig::default();
        let config2 = EmbeddingConfig {
            provider: "openai".to_string(),
            ..config1.clone()
        };

        let result = config1.validate_consistency(&config2);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Provider mismatch"));
    }

    #[test]
    fn test_validate_consistency_dimension_mismatch() {
        let config1 = EmbeddingConfig::default();
        let config2 = EmbeddingConfig {
            dimensions: 1536,
            ..config1.clone()
        };

        let result = config1.validate_consistency(&config2);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Dimension mismatch"));
    }
}
