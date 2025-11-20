//! Configuration management for the Guided Agent CLI.
//!
//! This module handles loading and merging configuration from multiple sources:
//! - Environment variables
//! - Command-line flags
//! - Config files (.guided/config.yaml)
//!
//! The configuration is workspace-centric, with most state stored in `.guided/`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::{AppError, AppResult};

/// Main application configuration.
///
/// This struct holds all global configuration options that affect
/// CLI behavior across commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Path to the workspace root (contains .guided/)
    pub workspace: PathBuf,

    /// Optional config file path
    pub config_file: Option<PathBuf>,

    /// Default LLM provider (e.g., "openai", "anthropic", "ollama")
    pub provider: String,

    /// Default model identifier
    pub model: String,

    /// API key for the LLM provider
    pub api_key: Option<String>,

    /// Log level override
    pub log_level: Option<String>,

    /// Verbose mode (enables debug logging)
    pub verbose: bool,

    /// Disable colored output
    pub no_color: bool,

    /// LLM provider configurations
    pub llm: Option<LlmConfig>,
}

/// LLM configuration from config.yaml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    #[serde(rename = "activeProvider")]
    pub active_provider: String,

    #[serde(rename = "activeEmbeddingProvider")]
    pub active_embedding_provider: String,

    pub providers: HashMap<String, ProviderConfig>,
}

/// Provider-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProviderConfig {
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
    Claude {
        #[serde(rename = "apiKeyEnv")]
        api_key_env: String,
        model: String,
        endpoint: Option<String>,
        #[serde(rename = "apiVersion")]
        api_version: Option<String>,
    },
    Ollama {
        endpoint: String,
        model: String,
        #[serde(rename = "embeddingModel")]
        embedding_model: Option<String>,
        timeout: Option<u64>,
    },
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

/// Full configuration file structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConfigFile {
    llm: Option<LlmConfig>,
    workspace: Option<WorkspaceConfig>,
    logging: Option<LoggingConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkspaceConfig {
    path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LoggingConfig {
    level: Option<String>,
    color: Option<bool>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            workspace: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            config_file: None,
            provider: "ollama".to_string(), // Local-first default
            model: "llama3.2".to_string(),
            api_key: None,
            log_level: None,
            verbose: false,
            no_color: false,
            llm: None,
        }
    }
}

impl AppConfig {
    /// Load configuration from environment variables and defaults.
    ///
    /// Environment variables:
    /// - `GUIDED_WORKSPACE`: Override workspace path
    /// - `GUIDED_CONFIG`: Path to config file
    /// - `GUIDED_PROVIDER`: LLM provider
    /// - `GUIDED_MODEL`: Model identifier
    /// - `GUIDED_API_KEY`: API key
    /// - `RUST_LOG`: Log level
    /// - `NO_COLOR`: Disable colored output
    ///
    /// # Example
    /// ```no_run
    /// use guided_core::config::AppConfig;
    ///
    /// let config = AppConfig::load().expect("Failed to load config");
    /// println!("Workspace: {:?}", config.workspace);
    /// ```
    pub fn load() -> AppResult<Self> {
        let mut config = Self::default();

        // Load from environment variables
        if let Ok(workspace) = std::env::var("GUIDED_WORKSPACE") {
            config.workspace = PathBuf::from(workspace);
        }

        if let Ok(config_file) = std::env::var("GUIDED_CONFIG") {
            config.config_file = Some(PathBuf::from(config_file));
        }

        // Validate workspace exists
        if !config.workspace.exists() {
            return Err(AppError::Config(format!(
                "Workspace directory does not exist: {:?}",
                config.workspace
            )));
        }

        // Load from YAML config file if it exists
        let config_path = if let Some(ref cf) = config.config_file {
            cf.clone()
        } else {
            config.workspace.join(".guided/config.yaml")
        };

        if config_path.exists() {
            config = config.merge_yaml(&config_path)?;
        }

        // Environment variables override YAML config
        if let Ok(provider) = std::env::var("GUIDED_PROVIDER") {
            config.provider = provider;
        }

        if let Ok(model) = std::env::var("GUIDED_MODEL") {
            config.model = model;
        }

        config.api_key = std::env::var("GUIDED_API_KEY").ok();
        config.log_level = std::env::var("RUST_LOG").ok();

        // Check for NO_COLOR environment variable
        if std::env::var("NO_COLOR").is_ok() {
            config.no_color = true;
        }

        Ok(config)
    }

    /// Merge YAML configuration file into this config.
    fn merge_yaml(&mut self, path: &PathBuf) -> AppResult<Self> {
        let contents = std::fs::read_to_string(path).map_err(|e| {
            AppError::Config(format!("Failed to read config file {:?}: {}", path, e))
        })?;

        let config_file: ConfigFile = serde_yaml::from_str(&contents).map_err(|e| {
            AppError::Config(format!("Failed to parse config file {:?}: {}", path, e))
        })?;

        let mut result = self.clone();

        // Merge workspace settings
        if let Some(ws) = config_file.workspace {
            if let Some(path) = ws.path {
                result.workspace = PathBuf::from(path);
            }
        }

        // Merge logging settings
        if let Some(logging) = config_file.logging {
            if let Some(level) = logging.level {
                result.log_level = Some(level);
            }
            if let Some(color) = logging.color {
                result.no_color = !color;
            }
        }

        // Merge LLM settings
        if let Some(llm) = config_file.llm {
            // Set active provider from YAML
            result.provider = llm.active_provider.clone();

            // Set model from active provider config
            if let Some(provider_config) = llm.providers.get(&llm.active_provider) {
                result.model = match provider_config {
                    ProviderConfig::OpenAI { model, .. } => model.clone(),
                    ProviderConfig::Claude { model, .. } => model.clone(),
                    ProviderConfig::Ollama { model, .. } => model.clone(),
                    ProviderConfig::GgufLocal { .. } => "gguf-local".to_string(),
                };
            }

            result.llm = Some(llm);
        }

        Ok(result)
    }

    /// Apply CLI overrides to the configuration.
    ///
    /// This method merges command-line flags with the loaded configuration,
    /// giving precedence to CLI flags over environment variables.
    #[allow(clippy::too_many_arguments)]
    pub fn with_overrides(
        mut self,
        workspace: Option<PathBuf>,
        config_file: Option<PathBuf>,
        provider: Option<String>,
        model: Option<String>,
        log_level: Option<String>,
        verbose: bool,
        no_color: bool,
    ) -> Self {
        if let Some(workspace) = workspace {
            self.workspace = workspace;
        }

        if let Some(config_file) = config_file {
            self.config_file = Some(config_file);
        }

        if let Some(provider) = provider {
            self.provider = provider;
        }

        if let Some(model) = model {
            self.model = model;
        }

        if let Some(log_level) = log_level {
            self.log_level = Some(log_level);
        }

        if verbose {
            self.verbose = true;
            // Verbose mode implies debug logging
            if self.log_level.is_none() {
                self.log_level = Some("debug".to_string());
            }
        }

        if no_color {
            self.no_color = true;
        }

        self
    }

    /// Get the path to the .guided directory.
    pub fn guided_dir(&self) -> PathBuf {
        self.workspace.join(".guided")
    }

    /// Ensure the .guided directory exists.
    pub fn ensure_guided_dir(&self) -> AppResult<()> {
        let guided_dir = self.guided_dir();
        if !guided_dir.exists() {
            std::fs::create_dir_all(&guided_dir).map_err(|e| {
                AppError::Config(format!("Failed to create .guided directory: {}", e))
            })?;
        }
        Ok(())
    }

    /// Get the active provider configuration.
    pub fn get_provider_config(&self, provider: &str) -> AppResult<Option<ProviderConfig>> {
        if let Some(ref llm) = self.llm {
            Ok(llm.providers.get(provider).cloned())
        } else {
            Ok(None)
        }
    }

    /// Resolve API key from environment variable.
    pub fn resolve_api_key(&self, provider: &str) -> AppResult<Option<String>> {
        // Check explicit GUIDED_API_KEY first
        if let Some(ref key) = self.api_key {
            return Ok(Some(key.clone()));
        }

        // Try provider-specific config
        if let Some(provider_config) = self.get_provider_config(provider)? {
            let env_var = match provider_config {
                ProviderConfig::OpenAI { api_key_env, .. } => Some(api_key_env),
                ProviderConfig::Claude { api_key_env, .. } => Some(api_key_env),
                _ => None,
            };

            if let Some(env_var) = env_var {
                if let Ok(key) = std::env::var(&env_var) {
                    return Ok(Some(key));
                }
            }
        }

        Ok(None)
    }

    /// Validate configuration for the active provider.
    pub fn validate(&self) -> AppResult<()> {
        // Check if provider is known
        let provider = &self.provider;
        let known_providers = ["openai", "claude", "ollama", "gguf-local"];

        if !known_providers.contains(&provider.as_str()) {
            return Err(AppError::Config(format!(
                "Unknown provider: {}. Supported: {}",
                provider,
                known_providers.join(", ")
            )));
        }

        // Validate provider-specific requirements
        if let Some(provider_config) = self.get_provider_config(provider)? {
            match provider_config {
                ProviderConfig::OpenAI { api_key_env, .. }
                | ProviderConfig::Claude { api_key_env, .. } => {
                    if std::env::var(&api_key_env).is_err() {
                        return Err(AppError::Config(format!(
                            "API key not found in environment variable: {}",
                            api_key_env
                        )));
                    }
                }
                ProviderConfig::GgufLocal { model_path_env, .. } => {
                    if let Ok(path) = std::env::var(&model_path_env) {
                        let model_path = PathBuf::from(path);
                        if !model_path.exists() {
                            return Err(AppError::Config(format!(
                                "GGUF model file not found: {:?}",
                                model_path
                            )));
                        }
                    } else {
                        return Err(AppError::Config(format!(
                            "Model path not found in environment variable: {}",
                            model_path_env
                        )));
                    }
                }
                ProviderConfig::Ollama { .. } => {
                    // Ollama doesn't require API keys
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.provider, "ollama");
        assert_eq!(config.model, "llama3.2");
        assert!(!config.verbose);
        assert!(!config.no_color);
    }

    #[test]
    fn test_guided_dir() {
        let config = AppConfig::default();
        let guided_dir = config.guided_dir();
        assert!(guided_dir.ends_with(".guided"));
    }

    #[test]
    fn test_with_overrides() {
        let config = AppConfig::default();
        let overridden = config.with_overrides(
            None,
            None,
            Some("openai".to_string()),
            Some("gpt-4".to_string()),
            None,
            true,
            false,
        );

        assert_eq!(overridden.provider, "openai");
        assert_eq!(overridden.model, "gpt-4");
        assert!(overridden.verbose);
        assert_eq!(overridden.log_level, Some("debug".to_string()));
    }

    #[test]
    fn test_validate_unknown_provider() {
        let mut config = AppConfig::default();
        config.provider = "unknown".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_ollama() {
        let mut config = AppConfig::default();
        config.provider = "ollama".to_string();
        assert!(config.validate().is_ok());
    }
}
