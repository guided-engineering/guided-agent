//! Filtered vector search for RAG
//!
//! Provides metadata-based filtering for vector similarity search to improve
//! retrieval quality and relevance.

use crate::types::KnowledgeChunk;
use chrono::{DateTime, Utc};
use guided_core::AppResult;
use serde::{Deserialize, Serialize};

/// Options for filtered vector search
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchFilters {
    /// Filter by file types (e.g., ["markdown", "code"])
    pub file_types: Option<Vec<String>>,

    /// Filter by languages (e.g., ["rust", "english"])
    pub languages: Option<Vec<String>>,

    /// Filter by tags (e.g., ["api", "docs"])
    pub tags: Option<Vec<String>>,

    /// Only include documents created after this timestamp
    pub created_after: Option<DateTime<Utc>>,

    /// Only include documents modified after this timestamp
    pub modified_after: Option<DateTime<Utc>>,

    /// Minimum relevance score (0.0 to 1.0)
    pub min_score: Option<f32>,

    /// Maximum number of results
    pub max_results: Option<usize>,
}

impl SearchFilters {
    /// Create a new empty filter set
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by file types
    pub fn with_file_types(mut self, file_types: Vec<String>) -> Self {
        self.file_types = Some(file_types);
        self
    }

    /// Filter by languages
    pub fn with_languages(mut self, languages: Vec<String>) -> Self {
        self.languages = Some(languages);
        self
    }

    /// Filter by tags
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = Some(tags);
        self
    }

    /// Filter by creation date
    pub fn with_created_after(mut self, created_after: DateTime<Utc>) -> Self {
        self.created_after = Some(created_after);
        self
    }

    /// Filter by modification date
    pub fn with_modified_after(mut self, modified_after: DateTime<Utc>) -> Self {
        self.modified_after = Some(modified_after);
        self
    }

    /// Set minimum relevance score
    pub fn with_min_score(mut self, min_score: f32) -> Self {
        self.min_score = Some(min_score);
        self
    }

    /// Set maximum results
    pub fn with_max_results(mut self, max_results: usize) -> Self {
        self.max_results = Some(max_results);
        self
    }

    /// Check if any filters are set
    pub fn has_filters(&self) -> bool {
        self.file_types.is_some()
            || self.languages.is_some()
            || self.tags.is_some()
            || self.created_after.is_some()
            || self.modified_after.is_some()
            || self.min_score.is_some()
    }

    /// Apply filters to a list of chunks with scores
    pub fn apply(&self, chunks: Vec<(KnowledgeChunk, f32)>) -> Vec<(KnowledgeChunk, f32)> {
        let mut filtered = chunks;

        // Filter by score first (most efficient)
        if let Some(min_score) = self.min_score {
            filtered.retain(|(_, score)| *score >= min_score);
        }

        // Filter by file types
        if let Some(file_types) = &self.file_types {
            filtered.retain(|(chunk, _)| {
                chunk
                    .metadata
                    .get("file_type")
                    .and_then(|v| v.as_str())
                    .map(|ft| file_types.iter().any(|t| ft.contains(t)))
                    .unwrap_or(false)
            });
        }

        // Filter by languages
        if let Some(languages) = &self.languages {
            filtered.retain(|(chunk, _)| {
                chunk
                    .metadata
                    .get("language")
                    .and_then(|v| v.as_str())
                    .map(|lang| languages.iter().any(|l| lang.eq_ignore_ascii_case(l)))
                    .unwrap_or(false)
            });
        }

        // Filter by tags (chunk must have at least one matching tag)
        if let Some(tags) = &self.tags {
            filtered.retain(|(chunk, _)| {
                chunk
                    .metadata
                    .get("tags")
                    .and_then(|v| v.as_array())
                    .map(|chunk_tags| {
                        chunk_tags.iter().any(|ct| {
                            ct.as_str()
                                .map(|ct_str| tags.iter().any(|t| ct_str.eq_ignore_ascii_case(t)))
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false)
            });
        }

        // Filter by creation date
        if let Some(created_after) = self.created_after {
            let created_after_ts = created_after.timestamp();
            filtered.retain(|(chunk, _)| {
                chunk
                    .metadata
                    .get("created_at")
                    .and_then(|v| v.as_i64())
                    .map(|ts| ts >= created_after_ts)
                    .unwrap_or(false)
            });
        }

        // Filter by modification date
        if let Some(modified_after) = self.modified_after {
            let modified_after_ts = modified_after.timestamp();
            filtered.retain(|(chunk, _)| {
                chunk
                    .metadata
                    .get("file_modified_at")
                    .and_then(|v| v.as_i64())
                    .map(|ts| ts >= modified_after_ts)
                    .unwrap_or(false)
            });
        }

        // Apply max results limit
        if let Some(max_results) = self.max_results {
            filtered.truncate(max_results);
        }

        filtered
    }
}

/// Detect query intent and generate default filters
pub fn detect_query_filters(query: &str) -> SearchFilters {
    let query_lower = query.to_lowercase();
    let mut filters = SearchFilters::new();

    // Detect if query is about code
    let code_indicators = ["function", "class", "method", "code", "implementation", "api"];
    if code_indicators.iter().any(|ind| query_lower.contains(ind)) {
        filters = filters.with_file_types(vec!["code".to_string()]);
    }

    // Detect if query is about documentation
    let doc_indicators = ["how to", "what is", "explain", "guide", "tutorial", "documentation"];
    if doc_indicators.iter().any(|ind| query_lower.contains(ind)) {
        filters = filters.with_file_types(vec!["markdown".to_string(), "text".to_string()]);
    }

    // Detect language preference (Portuguese indicators)
    let pt_indicators = ["como", "qual", "o que", "por que", "onde", "quando"];
    if pt_indicators.iter().any(|ind| query_lower.contains(ind)) {
        filters = filters.with_languages(vec!["portuguese".to_string()]);
    }

    filters
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_test_chunk(file_type: &str, language: &str, tags: Vec<&str>) -> KnowledgeChunk {
        KnowledgeChunk {
            id: "test".to_string(),
            source_id: "test".to_string(),
            position: 0,
            text: "test".to_string(),
            embedding: Some(vec![0.0; 384]),
            metadata: json!({
                "file_type": file_type,
                "language": language,
                "tags": tags,
                "created_at": 1700000000,
                "file_modified_at": 1700000000,
            }),
        }
    }

    #[test]
    fn test_filter_by_file_type() {
        let chunks = vec![
            (create_test_chunk("code", "rust", vec![]), 0.9),
            (create_test_chunk("markdown", "english", vec![]), 0.8),
            (create_test_chunk("code", "python", vec![]), 0.7),
        ];

        let filters = SearchFilters::new().with_file_types(vec!["code".to_string()]);
        let filtered = filters.apply(chunks);

        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|(c, _)| c
            .metadata
            .get("file_type")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("code")));
    }

    #[test]
    fn test_filter_by_language() {
        let chunks = vec![
            (create_test_chunk("code", "rust", vec![]), 0.9),
            (create_test_chunk("code", "python", vec![]), 0.8),
            (create_test_chunk("text", "english", vec![]), 0.7),
        ];

        let filters = SearchFilters::new().with_languages(vec!["rust".to_string()]);
        let filtered = filters.apply(chunks);

        assert_eq!(filtered.len(), 1);
        assert_eq!(
            filtered[0]
                .0
                .metadata
                .get("language")
                .unwrap()
                .as_str()
                .unwrap(),
            "rust"
        );
    }

    #[test]
    fn test_filter_by_tags() {
        let chunks = vec![
            (create_test_chunk("code", "rust", vec!["api", "utils"]), 0.9),
            (create_test_chunk("markdown", "english", vec!["docs"]), 0.8),
            (create_test_chunk("code", "python", vec!["api", "test"]), 0.7),
        ];

        let filters = SearchFilters::new().with_tags(vec!["api".to_string()]);
        let filtered = filters.apply(chunks);

        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_by_min_score() {
        let chunks = vec![
            (create_test_chunk("code", "rust", vec![]), 0.9),
            (create_test_chunk("code", "python", vec![]), 0.5),
            (create_test_chunk("text", "english", vec![]), 0.3),
        ];

        let filters = SearchFilters::new().with_min_score(0.6);
        let filtered = filters.apply(chunks);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].1, 0.9);
    }

    #[test]
    fn test_filter_max_results() {
        let chunks = vec![
            (create_test_chunk("code", "rust", vec![]), 0.9),
            (create_test_chunk("code", "python", vec![]), 0.8),
            (create_test_chunk("text", "english", vec![]), 0.7),
        ];

        let filters = SearchFilters::new().with_max_results(2);
        let filtered = filters.apply(chunks);

        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_detect_query_filters_code() {
        let query = "show me the function implementation";
        let filters = detect_query_filters(query);

        assert!(filters.file_types.is_some());
        assert_eq!(filters.file_types.unwrap()[0], "code");
    }

    #[test]
    fn test_detect_query_filters_docs() {
        let query = "how to use this library?";
        let filters = detect_query_filters(query);

        assert!(filters.file_types.is_some());
        let file_types = filters.file_types.unwrap();
        assert!(file_types.contains(&"markdown".to_string()));
    }

    #[test]
    fn test_detect_query_filters_portuguese() {
        let query = "como funciona o sistema?";
        let filters = detect_query_filters(query);

        assert!(filters.languages.is_some());
        assert_eq!(filters.languages.unwrap()[0], "portuguese");
    }

    #[test]
    fn test_no_filters_returns_all() {
        let chunks = vec![
            (create_test_chunk("code", "rust", vec![]), 0.9),
            (create_test_chunk("markdown", "english", vec![]), 0.8),
        ];

        let filters = SearchFilters::new();
        let filtered = filters.apply(chunks.clone());

        assert_eq!(filtered.len(), chunks.len());
    }
}
