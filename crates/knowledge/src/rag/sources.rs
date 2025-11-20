//! Source tracking for knowledge bases.
//!
//! Manages sources.jsonl file for tracking indexed sources.

use crate::types::KnowledgeSource;
use guided_core::{AppError, AppResult};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Manages source tracking for a knowledge base.
pub struct SourceManager {
    workspace: PathBuf,
    base_name: String,
}

impl SourceManager {
    /// Create a new source manager.
    pub fn new(workspace: &Path, base_name: &str) -> Self {
        Self {
            workspace: workspace.to_path_buf(),
            base_name: base_name.to_string(),
        }
    }

    /// Get path to sources.jsonl file.
    fn sources_path(&self) -> PathBuf {
        self.workspace
            .join(".guided")
            .join("knowledge")
            .join(&self.base_name)
            .join("sources.jsonl")
    }

    /// Track a new source by appending to sources.jsonl.
    pub fn track_source(&self, source: &KnowledgeSource) -> AppResult<()> {
        let sources_path = self.sources_path();

        // Ensure directory exists
        if let Some(parent) = sources_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Append to sources.jsonl (atomic write)
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&sources_path)
            .map_err(|e| {
                AppError::Knowledge(format!("Failed to open sources.jsonl: {}", e))
            })?;

        let json_line = serde_json::to_string(source)
            .map_err(|e| AppError::Knowledge(format!("Failed to serialize source: {}", e)))?;

        writeln!(file, "{}", json_line).map_err(|e| {
            AppError::Knowledge(format!("Failed to write to sources.jsonl: {}", e))
        })?;

        file.sync_all().map_err(|e| {
            AppError::Knowledge(format!("Failed to sync sources.jsonl: {}", e))
        })?;

        tracing::debug!("Tracked source: {:?}", source.path);
        Ok(())
    }

    /// List all tracked sources.
    pub fn list_sources(&self) -> AppResult<Vec<KnowledgeSource>> {
        let sources_path = self.sources_path();

        if !sources_path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&sources_path).map_err(|e| {
            AppError::Knowledge(format!("Failed to open sources.jsonl: {}", e))
        })?;

        let reader = BufReader::new(file);
        let mut sources = Vec::new();

        for (line_num, line) in reader.lines().enumerate() {
            let line = line.map_err(|e| {
                AppError::Knowledge(format!("Failed to read line {}: {}", line_num + 1, e))
            })?;

            if line.trim().is_empty() {
                continue;
            }

            let source: KnowledgeSource = serde_json::from_str(&line).map_err(|e| {
                AppError::Knowledge(format!(
                    "Failed to parse line {} in sources.jsonl: {}",
                    line_num + 1,
                    e
                ))
            })?;

            sources.push(source);
        }

        tracing::debug!("Listed {} sources from sources.jsonl", sources.len());
        Ok(sources)
    }

    /// Clear all tracked sources.
    pub fn clear_sources(&self) -> AppResult<()> {
        let sources_path = self.sources_path();

        if sources_path.exists() {
            std::fs::remove_file(&sources_path).map_err(|e| {
                AppError::Knowledge(format!("Failed to delete sources.jsonl: {}", e))
            })?;
            tracing::debug!("Cleared sources.jsonl");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_track_source_creates_jsonl() {
        let temp = TempDir::new().unwrap();
        let manager = SourceManager::new(temp.path(), "testbase");

        let source = KnowledgeSource {
            source_id: "test-id".to_string(),
            path: "test.md".to_string(),
            source_type: "file".to_string(),
            indexed_at: chrono::Utc::now(),
            chunk_count: 10,
            byte_count: 1024,
        };

        manager.track_source(&source).unwrap();

        let sources_path = manager.sources_path();
        assert!(sources_path.exists());
    }

    #[test]
    fn test_list_sources_parses_jsonl() {
        let temp = TempDir::new().unwrap();
        let manager = SourceManager::new(temp.path(), "testbase");

        let source1 = KnowledgeSource {
            source_id: "id1".to_string(),
            path: "test1.md".to_string(),
            source_type: "file".to_string(),
            indexed_at: chrono::Utc::now(),
            chunk_count: 5,
            byte_count: 512,
        };

        let source2 = KnowledgeSource {
            source_id: "id2".to_string(),
            path: "test2.md".to_string(),
            source_type: "file".to_string(),
            indexed_at: chrono::Utc::now(),
            chunk_count: 8,
            byte_count: 1024,
        };

        manager.track_source(&source1).unwrap();
        manager.track_source(&source2).unwrap();

        let sources = manager.list_sources().unwrap();
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].path, "test1.md");
        assert_eq!(sources[1].path, "test2.md");
    }

    #[test]
    fn test_clear_sources_deletes_file() {
        let temp = TempDir::new().unwrap();
        let manager = SourceManager::new(temp.path(), "testbase");

        let source = KnowledgeSource {
            source_id: "test-id".to_string(),
            path: "test.md".to_string(),
            source_type: "file".to_string(),
            indexed_at: chrono::Utc::now(),
            chunk_count: 10,
            byte_count: 1024,
        };

        manager.track_source(&source).unwrap();
        assert!(manager.sources_path().exists());

        manager.clear_sources().unwrap();
        assert!(!manager.sources_path().exists());
    }

    #[test]
    fn test_list_sources_empty_when_no_file() {
        let temp = TempDir::new().unwrap();
        let manager = SourceManager::new(temp.path(), "testbase");

        let sources = manager.list_sources().unwrap();
        assert_eq!(sources.len(), 0);
    }

    #[test]
    fn test_track_multiple_sources_appends() {
        let temp = TempDir::new().unwrap();
        let manager = SourceManager::new(temp.path(), "testbase");

        for i in 0..5 {
            let source = KnowledgeSource {
                source_id: format!("id{}", i),
                path: format!("test{}.md", i),
                source_type: "file".to_string(),
                indexed_at: chrono::Utc::now(),
                chunk_count: i as u32,
                byte_count: (i * 100) as u64,
            };
            manager.track_source(&source).unwrap();
        }

        let sources = manager.list_sources().unwrap();
        assert_eq!(sources.len(), 5);
    }
}
