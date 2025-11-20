//! Prompt loader for loading YAML prompt definitions.

use crate::types::PromptDefinition;
use guided_core::{AppError, AppResult};
use std::path::Path;

/// Load a prompt definition by ID from the workspace.
///
/// This function searches for a prompt file named `<id>.yml` in the
/// `.guided/prompts/` directory.
///
/// # Arguments
/// * `workspace_path` - Root workspace directory containing `.guided/`
/// * `prompt_id` - Prompt identifier (e.g., "agent.ask.default")
///
/// # Returns
/// A parsed `PromptDefinition` or an error if not found/invalid.
///
/// # Example
/// ```no_run
/// use guided_prompt::load_prompt;
/// use std::path::Path;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let workspace = Path::new(".");
/// let prompt = load_prompt(workspace, "agent.ask.default")?;
/// println!("Loaded prompt: {}", prompt.title);
/// # Ok(())
/// # }
/// ```
pub fn load_prompt(workspace_path: &Path, prompt_id: &str) -> AppResult<PromptDefinition> {
    let prompts_dir = workspace_path.join(".guided/prompts");
    let prompt_file = prompts_dir.join(format!("{}.yml", prompt_id));

    tracing::debug!("Loading prompt from: {:?}", prompt_file);

    if !prompt_file.exists() {
        return Err(AppError::Prompt(format!(
            "Prompt file not found: {:?}",
            prompt_file
        )));
    }

    let contents = std::fs::read_to_string(&prompt_file).map_err(|e| {
        AppError::Prompt(format!(
            "Failed to read prompt file {:?}: {}",
            prompt_file, e
        ))
    })?;

    let definition: PromptDefinition = serde_yaml::from_str(&contents).map_err(|e| {
        AppError::Prompt(format!(
            "Failed to parse prompt YAML {:?}: {}",
            prompt_file, e
        ))
    })?;

    // Validate required fields
    validate_prompt(&definition)?;

    tracing::info!("Loaded prompt: {} ({})", definition.id, definition.title);

    Ok(definition)
}

/// List all available prompt IDs in the workspace.
pub fn list_prompts(workspace_path: &Path) -> AppResult<Vec<String>> {
    let prompts_dir = workspace_path.join(".guided/prompts");

    if !prompts_dir.exists() {
        return Ok(Vec::new());
    }

    let mut prompt_ids = Vec::new();

    for entry in walkdir::WalkDir::new(&prompts_dir)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("yml") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                prompt_ids.push(stem.to_string());
            }
        }
    }

    Ok(prompt_ids)
}

/// Validate a prompt definition.
fn validate_prompt(def: &PromptDefinition) -> AppResult<()> {
    if def.id.is_empty() {
        return Err(AppError::Prompt("Prompt ID cannot be empty".to_string()));
    }

    if def.title.is_empty() {
        return Err(AppError::Prompt("Prompt title cannot be empty".to_string()));
    }

    if def.api_version.is_empty() {
        return Err(AppError::Prompt(
            "Prompt apiVersion cannot be empty".to_string(),
        ));
    }

    if def.template.is_empty() {
        return Err(AppError::Prompt(
            "Prompt template cannot be empty".to_string(),
        ));
    }

    // Validate API version format (simple check)
    if !def.api_version.contains('.') {
        return Err(AppError::Prompt(format!(
            "Invalid apiVersion format: {}. Expected format: 'x.y'",
            def.api_version
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_prompt(dir: &Path, id: &str, valid: bool) -> PathBuf {
        let prompts_dir = dir.join(".guided/prompts");
        fs::create_dir_all(&prompts_dir).unwrap();

        let content = if valid {
            format!(
                r#"
id: {}
title: "Test Prompt"
apiVersion: "1.0"
createdBy: test
behavior:
  tone: professional
  style: concise
context:
  includeWorkspaceContext: false
  includeKnowledgeBase: false
template: "Test template: {{{{prompt}}}}"
output:
  format: markdown
"#,
                id
            )
        } else {
            "invalid: yaml: content:".to_string()
        };

        let file_path = prompts_dir.join(format!("{}.yml", id));
        fs::write(&file_path, content).unwrap();
        file_path
    }

    #[test]
    fn test_load_valid_prompt() {
        let temp_dir = TempDir::new().unwrap();
        create_test_prompt(temp_dir.path(), "test.prompt", true);

        let result = load_prompt(temp_dir.path(), "test.prompt");
        assert!(result.is_ok());

        let prompt = result.unwrap();
        assert_eq!(prompt.id, "test.prompt");
        assert_eq!(prompt.title, "Test Prompt");
    }

    #[test]
    fn test_load_nonexistent_prompt() {
        let temp_dir = TempDir::new().unwrap();
        let result = load_prompt(temp_dir.path(), "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_load_invalid_yaml() {
        let temp_dir = TempDir::new().unwrap();
        create_test_prompt(temp_dir.path(), "invalid", false);

        let result = load_prompt(temp_dir.path(), "invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_prompts() {
        let temp_dir = TempDir::new().unwrap();
        create_test_prompt(temp_dir.path(), "prompt1", true);
        create_test_prompt(temp_dir.path(), "prompt2", true);

        let prompts = list_prompts(temp_dir.path()).unwrap();
        assert_eq!(prompts.len(), 2);
        assert!(prompts.contains(&"prompt1".to_string()));
        assert!(prompts.contains(&"prompt2".to_string()));
    }
}
