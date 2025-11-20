//! LLM configuration types.
//!
//! This module defines the configuration structures for LLM providers.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Complete LLM configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Active provider for completions
    #[serde(rename = "activeProvider")]
    pub active_provider: String,

    /// Active provider for embeddings
    #[serde(rename = "activeEmbeddingProvider")]
    pub active_embedding_provider: String,

    /// Provider-specific configurations
    pub providers: HashMap<String, LlmProviderConfig>,
}

impl Default for LlmConfig {
    fn default() -> Self {
        let mut providers = HashMap::new();

        // Ollama default
        providers.insert(
            "ollama".to_string(),
            LlmProviderConfig::Ollama {
                endpoint: "http://localhost:11434".to_string(),
                model: "llama3.2".to_string(),
                embedding_model: Some("nomic-embed-text".to_string()),
                timeout: Some(30),
            },
        );

        Self {
            active_provider: "ollama".to_string(),
            active_embedding_provider: "ollama".to_string(),
            providers,
        }
    }
}

/// Provider-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LlmProviderConfig {
    /// OpenAI configuration
    OpenAI {
        #[serde(rename = "apiKeyEnv")]
        api_key_env: String,
        model: String,
        #[serde(rename = "embeddingModel")]
        embedding_model: Option<String>,
        endpoint: Option<String>,
        #[serde(rename = "organizationEnv")]
        organization_env: Option<String>,
    },

    /// Claude/Anthropic configuration
    Claude {
        #[serde(rename = "apiKeyEnv")]
        api_key_env: String,
        model: String,
        endpoint: Option<String>,
        #[serde(rename = "apiVersion")]
        api_version: Option<String>,
    },

    /// Ollama configuration
    Ollama {
        endpoint: String,
        model: String,
        #[serde(rename = "embeddingModel")]
        embedding_model: Option<String>,
        timeout: Option<u64>,
    },

    /// GGUF local model configuration
    GgufLocal {
        #[serde(rename = "modelPathEnv")]
        model_path_env: String,
        #[serde(rename = "embeddingModelPathEnv")]
        embedding_model_path_env: Option<String>,
        threads: Option<u32>,
        #[serde(rename = "contextSize")]
        context_size: Option<u32>,
    },
}

impl LlmProviderConfig {
    /// Get the model name for this provider.
    pub fn model(&self) -> &str {
        match self {
            Self::OpenAI { model, .. } => model,
            Self::Claude { model, .. } => model,
            Self::Ollama { model, .. } => model,
            Self::GgufLocal { .. } => "gguf-local",
        }
    }

    /// Get the embedding model name if available.
    pub fn embedding_model(&self) -> Option<&str> {
        match self {
            Self::OpenAI {
                embedding_model, ..
            } => embedding_model.as_deref(),
            Self::Ollama {
                embedding_model, ..
            } => embedding_model.as_deref(),
            Self::GgufLocal { .. } => Some("gguf-embed"),
            Self::Claude { .. } => None,
        }
    }
}

/// Provider type enum for matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderType {
    OpenAI,
    Claude,
    Ollama,
    GgufLocal,
}

impl ProviderType {
    /// Parse provider type from string.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "openai" => Some(Self::OpenAI),
            "claude" | "anthropic" => Some(Self::Claude),
            "ollama" => Some(Self::Ollama),
            "gguf-local" | "gguf" => Some(Self::GgufLocal),
            _ => None,
        }
    }

    /// Get the canonical provider name.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OpenAI => "openai",
            Self::Claude => "claude",
            Self::Ollama => "ollama",
            Self::GgufLocal => "gguf-local",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_type_parsing() {
        assert_eq!(ProviderType::parse("openai"), Some(ProviderType::OpenAI));
        assert_eq!(ProviderType::parse("claude"), Some(ProviderType::Claude));
        assert_eq!(ProviderType::parse("anthropic"), Some(ProviderType::Claude));
        assert_eq!(ProviderType::parse("ollama"), Some(ProviderType::Ollama));
        assert_eq!(ProviderType::parse("gguf"), Some(ProviderType::GgufLocal));
        assert_eq!(ProviderType::parse("unknown"), None);
    }

    #[test]
    fn test_default_config() {
        let config = LlmConfig::default();
        assert_eq!(config.active_provider, "ollama");
        assert!(config.providers.contains_key("ollama"));
    }
}
