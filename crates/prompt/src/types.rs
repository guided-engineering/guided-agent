//! Prompt types for the Guided Agent CLI.
//!
//! This module defines the domain entities for the prompt system.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A prompt definition loaded from YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptDefinition {
    /// Unique prompt identifier
    pub id: String,

    /// Human-readable title
    pub title: String,

    /// API version for schema evolution
    #[serde(rename = "apiVersion")]
    pub api_version: String,

    /// Creator identifier
    #[serde(rename = "createdBy", default)]
    pub created_by: String,

    /// Behavioral settings
    pub behavior: PromptBehavior,

    /// Context injection settings
    pub context: PromptContextConfig,

    /// Input specification
    #[serde(default)]
    pub input: PromptInputSpec,

    /// Template string with Handlebars syntax
    pub template: String,

    /// Output specification
    pub output: PromptOutputSpec,
}

/// Behavioral settings for prompt execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptBehavior {
    /// Tone (e.g., "professional", "casual", "technical")
    pub tone: String,

    /// Style (e.g., "concise", "detailed", "conversational")
    pub style: String,
}

/// Context injection configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptContextConfig {
    /// Include workspace file tree and metadata
    #[serde(rename = "includeWorkspaceContext", default)]
    pub include_workspace_context: bool,

    /// Include knowledge base context
    #[serde(rename = "includeKnowledgeBase", default)]
    pub include_knowledge_base: bool,

    /// Optional knowledge base name
    #[serde(rename = "knowledgeBaseName", skip_serializing_if = "Option::is_none")]
    pub knowledge_base_name: Option<String>,
}

/// Input specification for the prompt.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PromptInputSpec {
    /// Description of the prompt field
    #[serde(default)]
    pub prompt: String,
}

/// Output specification for the prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptOutputSpec {
    /// Output format (e.g., "text", "markdown", "json")
    pub format: String,
}

/// A fully built prompt ready for LLM execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuiltPrompt {
    /// System message (optional)
    pub system: Option<String>,

    /// User message (required)
    pub user: String,

    /// Metadata about the built prompt
    pub metadata: BuiltPromptMetadata,
}

/// Metadata about a built prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuiltPromptMetadata {
    /// Source prompt ID
    #[serde(rename = "sourcePromptId")]
    pub source_prompt_id: String,

    /// Whether workspace context was included
    #[serde(rename = "workspaceContextIncluded")]
    pub workspace_context_included: bool,

    /// Knowledge base used (if any)
    #[serde(rename = "knowledgeBaseUsed", skip_serializing_if = "Option::is_none")]
    pub knowledge_base_used: Option<String>,

    /// Template variables that were resolved
    #[serde(rename = "resolvedVariables")]
    pub resolved_variables: HashMap<String, String>,
}

impl BuiltPrompt {
    /// Create a new built prompt.
    pub fn new(
        system: Option<String>,
        user: String,
        source_prompt_id: String,
        workspace_context_included: bool,
        knowledge_base_used: Option<String>,
        resolved_variables: HashMap<String, String>,
    ) -> Self {
        Self {
            system,
            user,
            metadata: BuiltPromptMetadata {
                source_prompt_id,
                workspace_context_included,
                knowledge_base_used,
                resolved_variables,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_definition_deserialization() {
        let yaml = r#"
id: test.prompt
title: Test Prompt
apiVersion: "1.0"
createdBy: test
behavior:
  tone: professional
  style: concise
context:
  includeWorkspaceContext: true
  includeKnowledgeBase: false
input:
  prompt: "User question"
template: "{{prompt}}"
output:
  format: markdown
"#;

        let def: PromptDefinition = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(def.id, "test.prompt");
        assert_eq!(def.behavior.tone, "professional");
        assert!(def.context.include_workspace_context);
        assert!(!def.context.include_knowledge_base);
    }

    #[test]
    fn test_built_prompt_creation() {
        let mut vars = HashMap::new();
        vars.insert("prompt".to_string(), "test".to_string());

        let built = BuiltPrompt::new(
            Some("System message".to_string()),
            "User message".to_string(),
            "test.prompt".to_string(),
            true,
            None,
            vars,
        );

        assert_eq!(built.system, Some("System message".to_string()));
        assert_eq!(built.user, "User message");
        assert_eq!(built.metadata.source_prompt_id, "test.prompt");
        assert!(built.metadata.workspace_context_included);
    }
}
