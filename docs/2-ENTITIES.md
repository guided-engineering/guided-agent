# Guided Agent CLI — Domain Entities

This document lists and describes the main **domain entities** of the Guided Agent CLI, across core, LLM, knowledge, task, prompt system, and stats.

---

## 1. Core Entities

### 1.1 `AppConfig`

**Role:** Central runtime configuration for the CLI.

**Fields (examples):**

* `workspacePath: PathBuf`
* `configFile: Option<PathBuf>`
* `provider: Option<String>`
* `model: Option<String>`
* `logLevel: LogLevel`

**Notes:**

* Loaded from flags, env, and optional config file.
* Passed down to other subsystems (LLM, knowledge, task).

---

### 1.2 `AppError`

**Role:** Unified error type for the CLI.

**Categories:**

* `ConfigError`
* `IoError`
* `LlMError`
* `KnowledgeError`
* `PromptError`
* `TaskError`
* `StatsError`

**Notes:**

* Used as the main error type in `Result<T, AppError>` across crates.

---

### 1.3 `LogLevel`

**Role:** Internal representation of logging level.

**Examples:**

* `Error`
* `Warn`
* `Info`
* `Debug`
* `Trace`

---

## 2. LLM Entities

### 2.1 `LLMClient`

**Role:** Trait representing a generic LLM client.

**Key Methods (conceptual):**

* `complete(request: LlmRequest) -> LlmResponse`
* `stream(request: LlmRequest) -> LlmStream`

---

### 2.2 `LlmRequest`

**Role:** Canonical request structure for LLM calls.

**Fields (examples):**

* `model: String`
* `systemPrompt: Option<String>`
* `userPrompt: String`
* `temperature: f32`
* `maxTokens: Option<u32>`
* `stream: bool`

---

### 2.3 `LlmResponse`

**Role:** Non-streaming response payload.

**Fields:**

* `text: String`
* `usage: Option<LlmUsage>`

---

### 2.4 `LlmUsage`

**Role:** Token and cost metadata.

**Fields:**

* `promptTokens: u32`
* `completionTokens: u32`
* `totalTokens: u32`

---

### 2.5 `LlmStreamChunk`

**Role:** Single chunk of a streaming response.

**Fields:**

* `text: String`
* `isFinal: bool`

---

## 3. Prompt System Entities

### 3.1 `PromptDefinition`

**Role:** Represents a prompt loaded from YAML in `.guided/prompts`.

**Fields (examples):**

* `id: String`
* `title: String`
* `apiVersion: String`
* `createdBy: String`
* `behavior: PromptBehavior`
* `context: PromptContextConfig`
* `input: PromptInputSpec`
* `template: String`
* `output: PromptOutputSpec`

---

### 3.2 `PromptBehavior`

**Role:** Controls tone and style.

**Fields:**

* `tone: String`
* `style: String`

---

### 3.3 `PromptContextConfig`

**Role:** Defines what context is injected.

**Fields:**

* `includeWorkspaceContext: bool`
* `includeKnowledgeBase: bool`
* `knowledgeBaseName: Option<String>`

---

### 3.4 `PromptInputSpec`

**Role:** Describes expected user input fields.

**Fields:**

* `prompt: String` (description)

---

### 3.5 `PromptOutputSpec`

**Role:** Describes the output format.

**Fields:**

* `format: String` (e.g. `text`, `markdown`, `json`)

---

### 3.6 `BuiltPrompt`

**Role:** Materialized prompt ready for LLM.

**Fields:**

* `system: Option<String>`
* `user: String`
* `metadata: BuiltPromptMetadata`

---

### 3.7 `BuiltPromptMetadata`

**Role:** Extra info about the built prompt.

**Fields:**

* `sourcePromptId: String`
* `workspaceContextIncluded: bool`
* `knowledgeBaseUsed: Option<String>`

---

## 4. Knowledge / RAG Entities

### 4.1 `KnowledgeBaseConfig`

**Role:** Configuration for a single knowledge base.

**Fields:**

* `name: String`
* `providerType: String`
* `model: String`
* `sqlitePath: PathBuf`
* `chunkSize: u32`
* `chunkOverlap: u32`
* `maxContextTokens: u32`

---

### 4.2 `KnowledgeSource`

**Role:** Describes an ingested source.

**Fields:**

* `id: String`
* `path: Option<PathBuf>`
* `url: Option<String>`
* `contentType: String`
* `learnedAt: DateTime`

---

### 4.3 `KnowledgeChunk`

**Role:** Single textual chunk stored in the index.

**Fields:**

* `id: String`
* `sourceId: String`
* `position: u32`
* `text: String`
* `embedding: Vec<f32>` (stored in SQLite)

---

### 4.4 `LearnOptions`

**Role:** Input options for `knowledge learn`.

**Fields:**

* `baseName: String`
* `path: Option<PathBuf>`
* `urls: Vec<String>`
* `include: Vec<String>`
* `exclude: Vec<String>`
* `reset: bool`

---

### 4.5 `LearnStats`

**Role:** Result summary of a learn operation.

**Fields:**

* `sourcesCount: u32`
* `chunksCount: u32`
* `bytesProcessed: u64`

---

### 4.6 `AskOptions`

**Role:** Input options for `knowledge ask`.

**Fields:**

* `baseName: String`
* `prompt: String`
* `topK: u32`

---

### 4.7 `AskResult`

**Role:** Result of a retrieval+LLM answer.

**Fields:**

* `answer: String`
* `chunks: Vec<KnowledgeChunk>`

---

### 4.8 `BaseStats`

**Role:** Aggregate statistics per base.

**Fields:**

* `baseName: String`
* `sourcesCount: u32`
* `chunksCount: u32`
* `dbSizeBytes: u64`
* `lastLearnAt: Option<DateTime>`

---

## 5. Task System Entities

### 5.1 `TaskId`

**Role:** Logical identifier for a task/plan.

**Representation:**

* String (e.g. `"feature-login"`, `"task-20251119-001"`).

---

### 5.2 `TaskPlan`

**Role:** Structured plan for a multi-step task.

**Fields:**

* `id: TaskId`
* `title: String`
* `description: String`
* `createdAt: DateTime`
* `steps: Vec<TaskStep>`

---

### 5.3 `TaskStep`

**Role:** Single plan step to be executed.

**Fields:**

* `id: String`
* `title: String`
* `description: String`
* `targetFiles: Vec<PathBuf>`
* `action: TaskStepAction`

---

### 5.4 `TaskStepAction`

**Role:** Discriminated union of possible actions.

**Variants (examples):**

* `CreateFile { path, contentTemplate }`
* `EditFile { path, instructions }`
* `DeleteFile { path }`
* `RunCommand { command, args }`

---

### 5.5 `TaskExecutionResult`

**Role:** Result of running a task.

**Fields:**

* `taskId: TaskId`
* `steps: Vec<TaskStepResult>`
* `startedAt: DateTime`
* `finishedAt: DateTime`

---

### 5.6 `TaskStepResult`

**Role:** Execution record per step.

**Fields:**

* `stepId: String`
* `status: TaskStepStatus`
* `message: Option<String>`
* `changedFiles: Vec<PathBuf>`

---

### 5.7 `TaskStepStatus`

**Role:** Status enum.

**Values:**

* `Pending`
* `Success`
* `Failed`
* `Skipped`

---

## 6. CLI / Command Entities

### 6.1 `CliCommand`

**Role:** High-level command enum.

**Variants:**

* `Ask(AskCommand)`
* `Task(TaskCommand)`
* `Knowledge(KnowledgeCommand)`
* `Stats(StatsCommand)`

---

### 6.2 `AskCommand`

**Fields:**

* `prompt: Option<String>` — Positional question text
* `prompt_flag: Option<String>` — Explicit --prompt flag
* `file: Option<PathBuf>` — Read prompt from file
* `knowledge_base: Option<String>` — Knowledge base name
* `with_workspace: bool` — Include workspace context
* `stream: bool` — Enable streaming (default: true)
* `no_stream: bool` — Disable streaming
* `max_tokens: Option<u32>` — Response token limit
* `temperature: Option<f32>` — Generation temperature
* `format: String` — Output format (default: markdown)
* `json: bool` — Output as JSON

---

### 6.3 `TaskCommand`

**Variants:**

* `Plan(TaskPlanCommand)`
* `Run(TaskRunCommand)`
* `Show(TaskShowCommand)`

---

### 6.4 `KnowledgeCommand`

**Variants:**

* `Learn(KnowledgeLearnCommand)`
* `Ask(KnowledgeAskCommand)`
* `Clean(KnowledgeCleanCommand)`
* `Stats(KnowledgeStatsCommand)`

---

### 6.5 `StatsCommand`

**Fields:**

* `json: bool`
* `detailed: bool`
* `reset: bool`

---

## 7. Stats & Telemetry Entities

### 7.1 `UsageStats`

**Role:** Aggregate usage stats stored in `.guided/stats.json`.

**Fields:**

* `commands: Vec<CommandStats>`
* `updatedAt: DateTime`

---

### 7.2 `CommandStats`

**Role:** Per-command counters.

**Fields:**

* `name: String` (e.g. `"ask"`, `"task.plan"`)
* `count: u64`
* `totalDurationMs: u64`

---

### 7.3 `LlMStats`

**Role:** Optional LLM usage stats.

**Fields:**

* `provider: String`
* `model: String`
* `totalTokens: u64`

---

## 8. Filesystem Contract Entities

### 8.1 `.guided/` Root Structure

* `prompts/` — prompt definitions.
* `tasks/` — task plans and execution logs.
* `knowledge/` — per-base knowledge indexes.
* `architecture/` — architecture/docs.
* `operation/` — worklog, changelog, stats.

---

### 8.2 `TaskPlanFile`

**Location:** `.guided/tasks/<task-id>.json`

**Contains:** serialized `TaskPlan`.

---

### 8.3 `TaskLogFile`

**Location:** `.guided/tasks/<task-id>.log.json`

**Contains:** serialized `TaskExecutionResult`.

---

### 8.4 `KnowledgeConfigFile`

**Location:** `.guided/knowledge/<base>/config.yaml`

**Contains:** serialized `KnowledgeBaseConfig`.

---

### 8.5 `KnowledgeSourcesFile`

**Location:** `.guided/knowledge/<base>/sources.jsonl`

**Contains:** `KnowledgeSource` entries (one per line).

---

### 8.6 `KnowledgeStatsFile`

**Location:** `.guided/knowledge/<base>/stats.json`

**Contains:** serialized `BaseStats`.

---

### 8.7 `UsageStatsFile`

**Location:** `.guided/operation/stats.json`

**Contains:** serialized `UsageStats`.

---

This entities document serves as the domain map of the Guided Agent CLI. Each entity should be reflected in code structures (structs/enums) and used consistently across crates and JSON/YAML formats.
