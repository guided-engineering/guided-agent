# Guided Agent CLI — Data Dictionary

This data dictionary defines **commands, options, and entities** used in the Guided Agent CLI. It links CLI-level concepts (commands/flags) with internal domain entities and persisted structures under `.guided/`.

---

## 1. Commands

### 1.1 Command: `ask`

**Description:** Ask an LLM a question, optionally using workspace and knowledge context.

**Syntax:**

* `guided-agent ask [GLOBAL_OPTIONS] [OPTIONS] "<TEXT>"`
* `guided-agent ask [GLOBAL_OPTIONS] [OPTIONS] --prompt "<TEXT>"`
* `guided-agent ask [GLOBAL_OPTIONS] [OPTIONS] --file <PATH>`

**Global Options (shared):**

* `-w, --workspace <PATH>` — Path to workspace root.
* `-c, --config <FILE>` — Path to config file.
* `--log-level <LEVEL>` — One of `error|warn|info|debug|trace`.
* `-v, --verbose` — Shortcut for `--log-level debug`.
* `--no-color` — Disable colored output.
* `--provider <ID>` — Override configured LLM provider.
* `--model <ID>` — Override configured model.

**Ask Options:**

* `<PROMPT>` (positional) — Question text. Maps to `AskCommand.prompt`.
* `--prompt <TEXT>` — Question text (explicit flag). Conflicts with positional. Maps to `AskCommand.prompt_flag`.
* `--file <PATH>`, `-f` — Read prompt from file. Maps to `AskCommand.file`.
* `--knowledge-base <NAME>`, `-k` — Knowledge base to use as context. Maps to `AskCommand.knowledge_base`.
* `--with-workspace` — Include workspace file tree in context. Maps to `AskCommand.with_workspace`.
* `--stream` — Enable streaming (default: true). Maps to `AskCommand.stream`.
* `--no-stream` — Disable streaming. Conflicts with `--stream`. Maps to `AskCommand.no_stream`.
* `--max-tokens <N>` — Response token limit. Maps to `AskCommand.max_tokens`.
* `--temperature <FLOAT>` — Creativity level (0.0-2.0). Maps to `AskCommand.temperature`.
* `--format <FORMAT>`, `-o` — Output format. Default: markdown. Maps to `AskCommand.format`.
* `--json` — Output answer + metadata as JSON. Maps to `AskCommand.json`.

**Prompt Resolution Priority:**
1. Positional argument: `guided ask "text"`
2. Explicit flag: `--prompt "text"`
3. File input: `--file path.txt`

**Entity Mappings:**

* CLI: `AskCommand`
* Prompt: `PromptDefinition` (e.g. `agent.ask.default`)
* Builder output: `BuiltPrompt`, `BuiltPromptMetadata`
* LLM: `LlmRequest`, `LlmResponse`, `LlmStreamChunk`, `Usage`
* Knowledge (optional): `KnowledgeChunk` (Phase 5)

**Output Modes:**

* **Plain Text (default):** Streams or buffers answer to stdout, logs to stderr.
* **JSON:** Structured output with answer, model, provider, usage, metadata.

**JSON Schema:**
```json
{
  "answer": "string",
  "model": "string",
  "provider": "string",
  "usage": {
    "promptTokens": "number",
    "completionTokens": "number",
    "totalTokens": "number"
  },
  "metadata": {
    "promptId": "string",
    "workspaceContext": "boolean",
    "knowledgeBase": "string | null"
  }
}
```

---

### 1.2 Command: `task`

**Description:** Plan and execute multi-step engineering tasks.

**Syntax:**

* `guided-agent task plan [OPTIONS]`
* `guided-agent task run [OPTIONS]`
* `guided-agent task show [OPTIONS]`

**Subcommand: `task plan`**

Options:

* `-p, --prompt <TEXT>` — Natural language description of the task.
* `--file <PATH>` — Read task description from file.
* `--id <TASK_ID>` — Explicit task identifier. Maps to `TaskPlan.id`.
* `--overwrite` — Overwrite existing plan with same ID.
* `--json` — Print resulting `TaskPlan` as JSON.

Entity Mappings:

* CLI: `TaskPlanCommand`
* Prompt: `PromptDefinition` (e.g. `agent.task.plan`)
* Plan: `TaskPlan`, `TaskStep`, `TaskStepAction`
* File: `.guided/tasks/<task-id>.json` (serialized `TaskPlan`)

**Subcommand: `task run`**

Options:

* `--id <TASK_ID>` — Required. ID of the plan to execute.
* `--dry-run` — Do not modify files, simulate actions.
* `--step <N>` — Execute a specific step only.
* `--until-step <N>` — Execute up to a specific step.
* `--json` — Output `TaskExecutionResult` as JSON.

Entity Mappings:

* CLI: `TaskRunCommand`
* Plan: `TaskPlan`
* Execution: `TaskExecutionResult`, `TaskStepResult`, `TaskStepStatus`
* Files:

  * `.guided/tasks/<task-id>.json` (plan)
  * `.guided/tasks/<task-id>.log.json` (execution result)

**Subcommand: `task show`**

Options:

* `--id <TASK_ID>` — Required task ID.
* `--json` — Print `TaskPlan` as JSON.

Entity Mappings:

* CLI: `TaskShowCommand`
* Plan: `TaskPlan`

---

### 1.3 Command: `knowledge`

**Description:** Manage local knowledge bases for RAG.

**Syntax:**

* `guided-agent knowledge learn <BASE> [OPTIONS]`
* `guided-agent knowledge ask <BASE> [OPTIONS]`
* `guided-agent knowledge clean <BASE> [OPTIONS]`
* `guided-agent knowledge stats <BASE> [OPTIONS]`

**Subcommand: `knowledge learn <BASE>`**

Options:

* `<BASE>` — Knowledge base name. Maps to `KnowledgeBaseConfig.name`.
* `--path <PATH>` — Root directory for learning.
* `--url <URL> ...` — One or more URLs to ingest.
* `--include <PATTERN> ...` — Glob patterns for inclusion.
* `--exclude <PATTERN> ...` — Glob patterns for exclusion.
* `--reset` — Drop existing index before learning.
* `--json` — Output `LearnStats` as JSON.

Entity Mappings:

* CLI: `KnowledgeLearnCommand`
* Config: `KnowledgeBaseConfig`
* Sources: `KnowledgeSource`
* Chunks: `KnowledgeChunk`
* Options: `LearnOptions`
* Result: `LearnStats`
* Files:

  * `.guided/knowledge/<base>/config.yaml`
  * `.guided/knowledge/<base>/index.sqlite`
  * `.guided/knowledge/<base>/sources.jsonl`
  * `.guided/knowledge/<base>/stats.json`

**Subcommand: `knowledge ask <BASE>`**

Options:

* `<BASE>` — Knowledge base name.
* `-p, --prompt <TEXT>` — Query text.
* `--file <PATH>` — Query from file.
* `--top-k <N>` — Number of chunks to retrieve.
* `--stream` / `--no-stream` — Streaming toggle.
* `--json` — Output answer + context as JSON.

Entity Mappings:

* CLI: `KnowledgeAskCommand`
* Options: `AskOptions`
* Retrieval: `KnowledgeChunk`
* Answer: `AskResult`

**Subcommand: `knowledge clean <BASE>`**

Options:

* `<BASE>` — Knowledge base name.
* `--force` — No confirmation.
* `--json` — JSON result (e.g. `{ "status": "ok" }`).

Entity Mappings:

* CLI: `KnowledgeCleanCommand`
* Config/index files removed for base.

**Subcommand: `knowledge stats <BASE>`**

Options:

* `<BASE>` — Knowledge base name.
* `--json` — Output `BaseStats` as JSON.

Entity Mappings:

* CLI: `KnowledgeStatsCommand`
* Stats: `BaseStats`

---

### 1.4 Command: `stats`

**Description:** Show global CLI usage statistics.

**Syntax:**

* `guided-agent stats [OPTIONS]`

Options:

* `--json` — Print `UsageStats` as JSON.
* `--detailed` — Show per-command breakdown.
* `--reset` — Reset stats file.

Entity Mappings:

* CLI: `StatsCommand`
* Stats: `UsageStats`, `CommandStats`, optionally `LlMStats`
* File: `.guided/operation/stats.json`

---

## 2. Global Options → Entity Mapping

* `--workspace <PATH>` → `AppConfig.workspacePath`
* `--config <FILE>` → `AppConfig.configFile`
* `--log-level <LEVEL>` → `AppConfig.logLevel`
* `--provider <ID>` → `AppConfig.provider`
* `--model <ID>` → `AppConfig.model`

These are resolved once at startup into `AppConfig` and passed down.

---

## 3. Entities (Summary Table)

### 3.1 Core

* `AppConfig` — Runtime configuration
* `AppError` — Unified error type
* `LogLevel` — Logging level enum

### 3.2 LLM

* `LLMClient` — Trait for providers
* `LlmRequest` — LLM request data
* `LlmResponse` — Full response
* `LlmUsage` — Token usage
* `LlmStreamChunk` — Streaming chunk

### 3.3 Prompt System

* `PromptDefinition` — YAML prompt schema
* `PromptBehavior` — Tone/style
* `PromptContextConfig` — Context flags
* `PromptInputSpec` — Input specs
* `PromptOutputSpec` — Output specs
* `BuiltPrompt` — Final prompt
* `BuiltPromptMetadata` — Build metadata

### 3.4 Knowledge

* `KnowledgeBaseConfig` — Base configuration
* `KnowledgeSource` — Ingested source
* `KnowledgeChunk` — Chunk entry
* `LearnOptions` — Learn input
* `LearnStats` — Learn result
* `AskOptions` — Ask input
* `AskResult` — Ask result
* `BaseStats` — Base statistics

### 3.5 Task System

* `TaskId` — String ID
* `TaskPlan` — Plan definition
* `TaskStep` — Single step
* `TaskStepAction` — Action enum
* `TaskExecutionResult` — Run result
* `TaskStepResult` — Step result
* `TaskStepStatus` — Status enum

### 3.6 CLI Command Types

* `CliCommand` — Top-level command enum
* `AskCommand` — Ask arguments
* `TaskCommand` — Task subcommands
* `KnowledgeCommand` — Knowledge subcommands
* `StatsCommand` — Stats arguments

### 3.7 Stats & Telemetry

* `UsageStats` — Overall usage
* `CommandStats` — Per-command stats
* `LlMStats` — LLM usage

---

## 4. Filesystem Mapping

* `.guided/prompts/*.yml` → `PromptDefinition`
* `.guided/tasks/<task-id>.json` → `TaskPlan`
* `.guided/tasks/<task-id>.log.json` → `TaskExecutionResult`
* `.guided/knowledge/<base>/config.yaml` → `KnowledgeBaseConfig`
* `.guided/knowledge/<base>/sources.jsonl` → `KnowledgeSource` entries
* `.guided/knowledge/<base>/stats.json` → `BaseStats`
* `.guided/operation/stats.json` → `UsageStats`

This dictionary should be kept in sync with the codebase and used as a reference when adding new commands, options, or entities.
