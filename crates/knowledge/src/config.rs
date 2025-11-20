//! Knowledge base configuration management.

use crate::types::KnowledgeBaseConfig;
use guided_core::{AppError, AppResult};
use std::fs;
use std::path::{Path, PathBuf};

/// Load knowledge base configuration.
///
/// Loads from `.guided/knowledge/<base>/config.yaml` if it exists,
/// otherwise creates a default config with the provided base name.
pub fn load_config(workspace: &Path, base_name: &str) -> AppResult<KnowledgeBaseConfig> {
    let config_path = get_config_path(workspace, base_name);

    if config_path.exists() {
        let content = fs::read_to_string(&config_path).map_err(|e| {
            AppError::Knowledge(format!("Failed to read config at {:?}: {}", config_path, e))
        })?;

        let mut config: KnowledgeBaseConfig = serde_yaml::from_str(&content).map_err(|e| {
            AppError::Knowledge(format!("Failed to parse config at {:?}: {}", config_path, e))
        })?;

        // Ensure name matches
        config.name = base_name.to_string();

        tracing::debug!("Loaded knowledge base config for '{}'", base_name);
        Ok(config)
    } else {
        // Create default config
        let config = KnowledgeBaseConfig {
            name: base_name.to_string(),
            ..Default::default()
        };

        tracing::debug!(
            "Using default knowledge base config for '{}' (no config file found)",
            base_name
        );
        Ok(config)
    }
}

/// Save knowledge base configuration.
pub fn save_config(workspace: &Path, config: &KnowledgeBaseConfig) -> AppResult<()> {
    let config_path = get_config_path(workspace, &config.name);

    // Ensure directory exists
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            AppError::Knowledge(format!("Failed to create config directory: {}", e))
        })?;
    }

    let yaml = serde_yaml::to_string(config)
        .map_err(|e| AppError::Knowledge(format!("Failed to serialize config: {}", e)))?;

    fs::write(&config_path, yaml).map_err(|e| {
        AppError::Knowledge(format!("Failed to write config to {:?}: {}", config_path, e))
    })?;

    tracing::debug!("Saved knowledge base config for '{}'", config.name);
    Ok(())
}

/// Get the path to a base's config file.
pub fn get_config_path(workspace: &Path, base_name: &str) -> PathBuf {
    workspace
        .join(".guided")
        .join("knowledge")
        .join(base_name)
        .join("config.yaml")
}

/// Get the base directory for a knowledge base.
pub fn get_base_dir(workspace: &Path, base_name: &str) -> PathBuf {
    workspace
        .join(".guided")
        .join("knowledge")
        .join(base_name)
}

/// Get the SQLite index path for a base.
pub fn get_index_path(workspace: &Path, base_name: &str) -> PathBuf {
    get_base_dir(workspace, base_name).join("index.sqlite")
}

/// Get the sources JSONL path for a base.
pub fn get_sources_path(workspace: &Path, base_name: &str) -> PathBuf {
    get_base_dir(workspace, base_name).join("sources.jsonl")
}

/// Get the stats JSON path for a base.
pub fn get_stats_path(workspace: &Path, base_name: &str) -> PathBuf {
    get_base_dir(workspace, base_name).join("stats.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_default_config() {
        let temp = TempDir::new().unwrap();
        let config = load_config(temp.path(), "test-base").unwrap();

        assert_eq!(config.name, "test-base");
        assert_eq!(config.provider, "ollama");
        assert_eq!(config.chunk_size, 512);
    }

    #[test]
    fn test_save_and_load_config() {
        let temp = TempDir::new().unwrap();
        let mut config = KnowledgeBaseConfig::default();
        config.name = "my-base".to_string();
        config.chunk_size = 1024;

        save_config(temp.path(), &config).unwrap();

        let loaded = load_config(temp.path(), "my-base").unwrap();
        assert_eq!(loaded.name, "my-base");
        assert_eq!(loaded.chunk_size, 1024);
    }
}
