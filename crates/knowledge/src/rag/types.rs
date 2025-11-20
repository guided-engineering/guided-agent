//! RAG response types.

use serde::{Deserialize, Serialize};

/// A single source reference used to answer a query.
///
/// This is the user-facing representation of where information came from.
/// Internal details like chunk IDs, scores, and byte offsets are hidden.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagSourceRef {
    /// Source file or document name (e.g., "gamedex.md", "playstore.html")
    pub source: String,

    /// Human-readable location within the source
    /// Examples: "lines 12-34", "developer section", "page 2"
    pub location: String,

    /// Short snippet showing the relevant evidence (truncated if needed)
    pub snippet: String,
}

/// Response from a RAG answering query.
///
/// Contains a natural language answer synthesized by an LLM,
/// along with human-readable source references.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagResponse {
    /// Natural language answer synthesized by the LLM
    pub answer: String,

    /// List of sources used to generate the answer
    pub sources: Vec<RagSourceRef>,

    /// Internal: highest similarity score from vector search
    /// Used for logging and confidence detection, not shown to users
    #[serde(skip_serializing)]
    pub max_score: f32,

    /// Internal: whether the answer has low confidence
    /// Used to trigger cautious answering behavior
    #[serde(skip_serializing)]
    pub low_confidence: bool,
}

impl RagResponse {
    /// Create a new RAG response.
    pub fn new(answer: String, sources: Vec<RagSourceRef>, max_score: f32) -> Self {
        let low_confidence = max_score < CONFIDENCE_THRESHOLD;

        Self {
            answer,
            sources,
            max_score,
            low_confidence,
        }
    }

    /// Create a "no information" response when no relevant chunks are found.
    pub fn no_information(query: &str) -> Self {
        Self {
            answer: format!(
                "I could not find information about \"{}\" in the available documents.",
                query
            ),
            sources: Vec::new(),
            max_score: 0.0,
            low_confidence: true,
        }
    }
}

/// Minimum score for high-confidence answering.
/// Scores below this trigger cautious/uncertain language in the LLM prompt.
pub const CONFIDENCE_THRESHOLD: f32 = 0.30;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rag_response_high_confidence() {
        let sources = vec![RagSourceRef {
            source: "test.md".to_string(),
            location: "lines 1-10".to_string(),
            snippet: "Test content".to_string(),
        }];

        let response = RagResponse::new("Test answer".to_string(), sources, 0.85);

        assert_eq!(response.answer, "Test answer");
        assert_eq!(response.sources.len(), 1);
        assert_eq!(response.max_score, 0.85);
        assert!(!response.low_confidence);
    }

    #[test]
    fn test_rag_response_low_confidence() {
        let sources = vec![RagSourceRef {
            source: "test.md".to_string(),
            location: "lines 1-10".to_string(),
            snippet: "Test content".to_string(),
        }];

        let response = RagResponse::new("Test answer".to_string(), sources, 0.25);

        assert!(response.low_confidence);
        assert_eq!(response.max_score, 0.25);
    }

    #[test]
    fn test_no_information_response() {
        let response = RagResponse::no_information("test query");

        assert!(response.answer.contains("test query"));
        assert!(response.answer.contains("could not find"));
        assert!(response.sources.is_empty());
        assert!(response.low_confidence);
        assert_eq!(response.max_score, 0.0);
    }

    #[test]
    fn test_source_ref_serialization() {
        let source_ref = RagSourceRef {
            source: "test.md".to_string(),
            location: "lines 1-10".to_string(),
            snippet: "Test snippet".to_string(),
        };

        let json = serde_json::to_string(&source_ref).unwrap();
        let deserialized: RagSourceRef = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.source, source_ref.source);
        assert_eq!(deserialized.location, source_ref.location);
        assert_eq!(deserialized.snippet, source_ref.snippet);
    }
}
