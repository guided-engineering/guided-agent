//! LLM provider factory.
//!
//! This module provides a factory for creating LLM clients based on
//! application configuration. It handles provider resolution, secret
//! injection, and basic health checks.

use crate::client::LlmClient;
use crate::providers::OllamaClient;
use std::sync::Arc;

/// Create an LLM client based on the provider name.
///
/// This function performs the following:
/// 1. Matches the provider string to a known provider type
/// 2. Resolves required secrets from environment variables
/// 3. Creates the appropriate client implementation
/// 4. Optionally performs health checks
///
/// # Arguments
/// * `provider` - Provider identifier ("openai", "claude", "ollama", "gguf-local")
/// * `endpoint` - Optional custom endpoint URL
/// * `api_key` - Optional API key (for providers that require it)
///
/// # Returns
/// A boxed trait object implementing `LlmClient`
///
/// # Errors
/// Returns error if:
/// - Provider is unknown
/// - Required secrets are missing
/// - Client initialization fails
pub fn create_client(
    provider: &str,
    endpoint: Option<&str>,
    api_key: Option<&str>,
) -> Result<Arc<dyn LlmClient>, String> {
    match provider.to_lowercase().as_str() {
        "ollama" => {
            let base_url = endpoint.unwrap_or("http://localhost:11434");
            let client = OllamaClient::with_base_url(base_url);
            Ok(Arc::new(client))
        }
        "openai" => {
            if api_key.is_none() {
                return Err("OpenAI provider requires API key".to_string());
            }
            // TODO: Implement OpenAI client
            Err("OpenAI provider not yet implemented".to_string())
        }
        "claude" | "anthropic" => {
            if api_key.is_none() {
                return Err("Claude provider requires API key".to_string());
            }
            // TODO: Implement Claude client
            Err("Claude provider not yet implemented".to_string())
        }
        "gguf-local" | "gguf" => {
            // TODO: Implement GGUF client
            Err("GGUF provider not yet implemented".to_string())
        }
        _ => Err(format!("Unknown provider: {}", provider)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_ollama_client() {
        let client = create_client("ollama", None, None);
        assert!(client.is_ok());
    }

    #[test]
    fn test_create_ollama_with_custom_endpoint() {
        let client = create_client("ollama", Some("http://localhost:8080"), None);
        assert!(client.is_ok());
    }

    #[test]
    fn test_openai_requires_api_key() {
        match create_client("openai", None, None) {
            Err(err) => assert!(err.contains("OpenAI provider requires API key")),
            Ok(_) => panic!("Expected error for OpenAI without API key"),
        }
    }

    #[test]
    fn test_claude_requires_api_key() {
        match create_client("claude", None, None) {
            Err(err) => assert!(err.contains("Claude provider requires API key")),
            Ok(_) => panic!("Expected error for Claude without API key"),
        }
    }

    #[test]
    fn test_unknown_provider() {
        match create_client("unknown", None, None) {
            Err(err) => assert!(err.contains("Unknown provider")),
            Ok(_) => panic!("Expected error for unknown provider"),
        }
    }
}
