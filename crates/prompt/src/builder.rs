//! Prompt builder for rendering templates and injecting context.

use crate::types::{BuiltPrompt, PromptDefinition};
use guided_core::{AppError, AppResult};
use handlebars::Handlebars;
use std::collections::HashMap;
use std::path::Path;

/// Build a prompt from a definition and input variables.
///
/// This function:
/// 1. Renders the template using Handlebars with provided variables
/// 2. Injects workspace context if enabled
/// 3. Injects knowledge base context if enabled
/// 4. Returns a `BuiltPrompt` ready for LLM execution
///
/// # Arguments
/// * `definition` - Prompt definition loaded from YAML
/// * `variables` - Template variables (e.g., "prompt" -> user input)
/// * `workspace_path` - Path to workspace (for context injection)
/// * `knowledge_context` - Optional knowledge base context
///
/// # Example
/// ```no_run
/// use guided_prompt::{build_prompt, PromptDefinition};
/// use std::collections::HashMap;
/// use std::path::Path;
///
/// # fn example(def: PromptDefinition) -> Result<(), Box<dyn std::error::Error>> {
/// let mut vars = HashMap::new();
/// vars.insert("prompt".to_string(), "What is Rust?".to_string());
///
/// let built = build_prompt(&def, vars, Path::new("."), None)?;
/// println!("User prompt: {}", built.user);
/// # Ok(())
/// # }
/// ```
pub fn build_prompt(
    definition: &PromptDefinition,
    mut variables: HashMap<String, String>,
    workspace_path: &Path,
    knowledge_context: Option<String>,
) -> AppResult<BuiltPrompt> {
    tracing::debug!("Building prompt: {}", definition.id);

    // Inject workspace context if enabled
    let workspace_context_included = definition.context.include_workspace_context;
    if workspace_context_included {
        let workspace_ctx = generate_workspace_context(workspace_path)?;
        variables.insert("workspaceContext".to_string(), workspace_ctx);
        tracing::debug!("Injected workspace context");
    }

    // Inject knowledge context if enabled
    let knowledge_base_used = if definition.context.include_knowledge_base {
        if let Some(kb_ctx) = knowledge_context {
            variables.insert("knowledgeContext".to_string(), kb_ctx);
            tracing::debug!("Injected knowledge base context");
            definition.context.knowledge_base_name.clone()
        } else {
            tracing::warn!("Knowledge base context requested but not provided");
            None
        }
    } else {
        None
    };

    // Render template using Handlebars
    let rendered = render_template(&definition.template, &variables)?;

    // Split into system and user messages
    // For now, entire template is user message
    // Future: support explicit system/user sections in template
    let system = None;
    let user = rendered;

    Ok(BuiltPrompt::new(
        system,
        user,
        definition.id.clone(),
        workspace_context_included,
        knowledge_base_used,
        variables,
    ))
}

/// Render a Handlebars template with variables.
fn render_template(template: &str, variables: &HashMap<String, String>) -> AppResult<String> {
    let mut handlebars = Handlebars::new();

    // Disable HTML escaping for plain text
    handlebars.register_escape_fn(handlebars::no_escape);

    // Register template
    handlebars
        .register_template_string("prompt", template)
        .map_err(|e| AppError::Prompt(format!("Failed to register template: {}", e)))?;

    // Render
    let rendered = handlebars
        .render("prompt", &variables)
        .map_err(|e| AppError::Prompt(format!("Failed to render template: {}", e)))?;

    Ok(rendered)
}

/// Generate workspace context summary.
///
/// This includes:
/// - File tree (top-level overview)
/// - Workspace metadata
fn generate_workspace_context(workspace_path: &Path) -> AppResult<String> {
    let mut context = String::new();

    context.push_str("# Workspace Context\n\n");
    context.push_str(&format!("Path: {}\n\n", workspace_path.display()));

    // Generate file tree (simplified)
    context.push_str("## File Structure\n\n");
    context.push_str("```\n");

    let tree = generate_file_tree(workspace_path, 2)?;
    context.push_str(&tree);

    context.push_str("```\n");

    Ok(context)
}

/// Generate a simple file tree.
fn generate_file_tree(path: &Path, max_depth: usize) -> AppResult<String> {
    let mut output = String::new();

    for entry in walkdir::WalkDir::new(path)
        .max_depth(max_depth)
        .into_iter()
        .filter_entry(|e| {
            // Skip hidden files and common exclude directories
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.') && name != "target" && name != "node_modules" && name != "dist"
        })
        .filter_map(|e| e.ok())
    {
        let depth = entry.depth();
        let indent = "  ".repeat(depth);
        let name = entry.file_name().to_string_lossy();

        if entry.file_type().is_dir() {
            output.push_str(&format!("{}{}/\n", indent, name));
        } else {
            output.push_str(&format!("{}{}\n", indent, name));
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{PromptBehavior, PromptContextConfig, PromptInputSpec, PromptOutputSpec};

    fn create_test_definition(include_workspace: bool, include_kb: bool) -> PromptDefinition {
        PromptDefinition {
            id: "test.prompt".to_string(),
            title: "Test".to_string(),
            api_version: "1.0".to_string(),
            created_by: "test".to_string(),
            behavior: PromptBehavior {
                tone: "professional".to_string(),
                style: "concise".to_string(),
            },
            context: PromptContextConfig {
                include_workspace_context: include_workspace,
                include_knowledge_base: include_kb,
                knowledge_base_name: Some("test-kb".to_string()),
            },
            input: PromptInputSpec::default(),
            template: "Question: {{prompt}}".to_string(),
            output: PromptOutputSpec {
                format: "markdown".to_string(),
            },
        }
    }

    #[test]
    fn test_render_simple_template() {
        let mut vars = HashMap::new();
        vars.insert("prompt".to_string(), "Hello, world!".to_string());

        let result = render_template("Question: {{prompt}}", &vars);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Question: Hello, world!");
    }

    #[test]
    fn test_build_prompt_without_context() {
        let def = create_test_definition(false, false);
        let mut vars = HashMap::new();
        vars.insert("prompt".to_string(), "Test question".to_string());

        let result = build_prompt(&def, vars, Path::new("."), None);
        assert!(result.is_ok());

        let built = result.unwrap();
        assert_eq!(built.user, "Question: Test question");
        assert!(!built.metadata.workspace_context_included);
        assert_eq!(built.metadata.knowledge_base_used, None);
    }

    #[test]
    fn test_build_prompt_with_knowledge_context() {
        let def = create_test_definition(false, true);
        let mut vars = HashMap::new();
        vars.insert("prompt".to_string(), "Test question".to_string());

        let kb_context = "Knowledge: Rust is a systems programming language.".to_string();
        let result = build_prompt(&def, vars, Path::new("."), Some(kb_context));
        assert!(result.is_ok());

        let built = result.unwrap();
        assert_eq!(
            built.metadata.knowledge_base_used,
            Some("test-kb".to_string())
        );
    }

    #[test]
    fn test_render_template_missing_variable() {
        let vars = HashMap::new();
        let result = render_template("Question: {{missing}}", &vars);
        // Handlebars renders missing variables as empty string
        assert!(result.is_ok());
    }
}
