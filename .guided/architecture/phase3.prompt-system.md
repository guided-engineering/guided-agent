# Phase 3: Prompt System Architecture

## Overview

The Prompt System provides structured, reproducible prompt management for the Guided Agent CLI. It enables:

- **YAML-based prompt definitions** stored in `.guided/prompts/`
- **Handlebars template rendering** with variable substitution
- **Context injection** (workspace files, knowledge base)
- **Metadata tracking** for reproducibility

This phase completes the architectural triangle: **Config → Provider → Prompt**.

---

## Architecture Components

### 1. Prompt Types (`crates/prompt/src/types.rs`)

#### `PromptDefinition`
The canonical representation of a prompt loaded from YAML.

```rust
pub struct PromptDefinition {
    pub id: String,                      // e.g., "agent.ask.default"
    pub title: String,                   // Human-readable title
    pub api_version: String,             // Schema version "1.0"
    pub created_by: String,              // Creator identifier
    pub behavior: PromptBehavior,        // Tone and style
    pub context: PromptContextConfig,    // Context injection settings
    pub input: PromptInputSpec,          // Input field descriptions
    pub template: String,                // Handlebars template
    pub output: PromptOutputSpec,        // Output format
}
```

#### `BuiltPrompt`
The materialized prompt ready for LLM execution.

```rust
pub struct BuiltPrompt {
    pub system: Option<String>,          // System message (optional)
    pub user: String,                    // User message (required)
    pub metadata: BuiltPromptMetadata,   // Build metadata
}
```

#### `BuiltPromptMetadata`
Tracks how the prompt was built for reproducibility.

```rust
pub struct BuiltPromptMetadata {
    pub source_prompt_id: String,
    pub workspace_context_included: bool,
    pub knowledge_base_used: Option<String>,
    pub resolved_variables: HashMap<String, String>,
}
```

---

### 2. Prompt Loader (`crates/prompt/src/loader.rs`)

#### `load_prompt(workspace_path, prompt_id)`
Loads and validates a prompt definition from `.guided/prompts/<id>.yml`.

**Validation Rules:**
- `id` must be non-empty
- `title` must be non-empty
- `apiVersion` must follow `x.y` format
- `template` must be non-empty

**Error Handling:**
- Returns `AppError::Prompt` if file not found, parse fails, or validation fails

#### `list_prompts(workspace_path)`
Lists all available prompt IDs in the workspace.

---

### 3. Prompt Builder (`crates/prompt/src/builder.rs`)

#### `build_prompt(definition, variables, workspace_path, knowledge_context)`
Renders a prompt template and injects context.

**Pipeline:**

```
1. Inject workspace context (if enabled)
   ↓
2. Inject knowledge base context (if enabled)
   ↓
3. Render Handlebars template with all variables
   ↓
4. Return BuiltPrompt with metadata
```

**Template Rendering:**
- Uses Handlebars with HTML escaping disabled (plain text)
- Variables: `{{prompt}}`, `{{workspaceContext}}`, `{{knowledgeContext}}`
- Conditionals: `{{#if workspaceContext}}...{{/if}}`
- Triple braces for no escaping: `{{{variable}}}`

**Context Generation:**

**Workspace Context:**
```markdown
# Workspace Context

Path: /path/to/workspace

## File Structure

```
crates/
  core/
  cli/
  llm/
  prompt/
docs/
```
```

**Knowledge Context:**
- Provided by caller (knowledge base retrieval)
- Injected as `{{knowledgeContext}}` variable

---

## Prompt Definition Schema

### Example: `agent.ask.default.yml`

```yaml
id: agent.ask.default
title: "Default Ask Prompt"
apiVersion: "1.0"
createdBy: guided-agent

behavior:
  tone: professional
  style: concise

context:
  includeWorkspaceContext: false
  includeKnowledgeBase: false

input:
  prompt: "User's question or request"

template: |
  {{#if workspaceContext}}
  {{{workspaceContext}}}
  
  {{/if}}
  {{#if knowledgeContext}}
  ## Relevant Knowledge
  
  {{{knowledgeContext}}}
  
  {{/if}}
  ## Question
  
  {{prompt}}

output:
  format: markdown
```

---

## Integration with Ask Command

### Flow

```
1. User runs: guided ask "What is Rust?"
   ↓
2. AskCommand loads prompt definition: "agent.ask.default"
   ↓
3. Build variables: {"prompt": "What is Rust?"}
   ↓
4. Call build_prompt() with variables and optional context
   ↓
5. BuiltPrompt returned with rendered template
   ↓
6. Create LlmRequest with built.user (and optional built.system)
   ↓
7. Send to LLM provider via factory
   ↓
8. Stream or return response to user
```

### Code Example

```rust
// Load prompt definition
let prompt_def = load_prompt(&config.workspace, "agent.ask.default")?;

// Build variables
let mut variables = HashMap::new();
variables.insert("prompt".to_string(), user_input);

// Build prompt with optional context
let built_prompt = build_prompt(
    &prompt_def,
    variables,
    &config.workspace,
    knowledge_context,
)?;

// Create LLM request
let mut request = LlmRequest::new(built_prompt.user, &config.model);
if let Some(system) = built_prompt.system {
    request = request.with_system(system);
}

// Execute
let response = client.complete(&request).await?;
```

---

## Configuration Precedence

**Prompt Loading:**
1. Load from `.guided/prompts/<id>.yml`
2. Validate schema
3. Return `PromptDefinition`

**Context Injection:**
1. Check `definition.context.includeWorkspaceContext`
2. Check `definition.context.includeKnowledgeBase`
3. Inject if enabled

**Variable Resolution:**
1. User-provided variables (e.g., `prompt`)
2. Injected context (e.g., `workspaceContext`, `knowledgeContext`)
3. Render template with all variables

---

## Performance Targets

- **Prompt loading**: < 10ms for YAML parse and validation
- **Context generation**: < 50ms for workspace file tree
- **Template rendering**: < 5ms for typical templates
- **Total build time**: < 100ms from load to BuiltPrompt

---

## Error Handling

All prompt operations return `AppResult<T>` with `AppError::Prompt` variant:

- **File not found**: `"Prompt file not found: <path>"`
- **Invalid YAML**: `"Failed to parse prompt YAML: <error>"`
- **Validation failure**: `"Prompt <field> cannot be empty"`
- **Template error**: `"Failed to render template: <error>"`

---

## Testing Strategy

### Unit Tests
- **types.rs**: Serialization/deserialization, BuiltPrompt creation
- **loader.rs**: Valid/invalid YAML, missing files, validation
- **builder.rs**: Template rendering, context injection, variable resolution

### Integration Tests
- Load real prompts from `.guided/prompts/`
- Build prompts with actual workspace context
- Verify Handlebars rendering with complex templates

### Test Coverage
- 10 unit tests in prompt crate
- 2 doc tests
- All tests passing ✓

---

## Future Enhancements (Phase 4+)

1. **System/User Split**: Explicit system and user sections in templates
2. **Multi-turn**: Support for conversation history
3. **Prompt Variants**: A/B testing with multiple templates
4. **Caching**: Cache rendered prompts for repeated queries
5. **Validation**: Schema validation with jsonschema
6. **Prompt Editor**: CLI command to create/edit prompts interactively

---

## Files Created/Modified

### New Files
- `crates/prompt/Cargo.toml`
- `crates/prompt/src/lib.rs`
- `crates/prompt/src/types.rs`
- `crates/prompt/src/loader.rs`
- `crates/prompt/src/builder.rs`
- `.guided/prompts/agent.ask.default.yml`

### Modified Files
- `Cargo.toml` (added prompt crate to workspace)
- `crates/cli/Cargo.toml` (added guided-prompt dependency)
- `crates/cli/src/commands/ask.rs` (integrated prompt system)

---

## Summary

Phase 3 delivers a complete, production-ready prompt system that:

✅ **Structured Definitions**: YAML-based prompts with schema validation  
✅ **Template Rendering**: Handlebars with variable substitution  
✅ **Context Injection**: Workspace and knowledge base context  
✅ **Metadata Tracking**: Full reproducibility of prompt builds  
✅ **Type Safety**: Strong Rust types throughout  
✅ **Error Handling**: Comprehensive error messages  
✅ **Testing**: 10 unit tests, all passing  
✅ **Integration**: Fully integrated with ask command  

The prompt system is now the authoritative layer for all LLM interactions, enabling consistent, reproducible, and context-aware AI assistance.
