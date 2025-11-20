# Guided Agent CLI — Technical Specification (SPEC)

## 1. Introduction

This SPEC defines the **non-functional requirements**, **architectural structure**, **command flows**, **technical standards**, and **stack decisions** for the Guided Agent CLI.
It complements the PRD and establishes the engineering-level contract for the implementation.

---

# 2. Non-Functional Requirements

## 2.1 Performance

* All commands must initialize in **< 120ms** on a typical laptop (no warm-up steps).
* Streaming responses must begin in **< 300ms** after sending request to the LLM provider.
* Knowledge retrieval (top‑k) must respond in **< 150ms** for 50k chunks.

## 2.2 Reliability

* CLI must never panic under normal operation.
* All errors must be represented using unified error types (`AppError`).
* Any file write operations must be atomic (write → fsync → rename).

## 2.3 Stability

* CLI stdout output must remain **stable and predictable**.
* Logs must go to stderr, results must go to stdout.
* JSON outputs must follow documented schemas.

## 2.4 Observability

* All commands must emit tracing spans:

  * `command.start`
  * `command.end`
  * `llm.request`
  * `knowledge.learn`
  * `knowledge.retrieve`
  * `task.plan`
  * `task.run`

## 2.5 Security

* No external network calls except:

  * LLM provider
  * URLs explicitly passed by user in `knowledge learn`.
* No sending workspace content to providers unless explicitly triggered.

## 2.6 Portability

* CLI must run on:

  * macOS (Intel + ARM)
  * Linux (x86_64 + ARM)
* No platform-specific dependencies unless optional.

---

# 3. High-Level Architecture

```
+----------------------------+
|       CLI (clap)          |
+------------+---------------+
             |
             v
+----------------------------+
|         Core Crate        |
| config, logging, errors   |
+------------+---------------+
             |
             +-------------------------+
             |                         |
             v                         v
+----------------------+   +-------------------------+
| LLM Providers (llm)  |   | Knowledge Base (rag)    |
| traits + adapters    |   | sqlite + embeddings     |
+----------+-----------+   +-----------+-------------+
           |                           |
           v                           v
+----------------------+   +-------------------------+
| Prompt Builder       |   | Task Engine             |
| templates + context  |   | plans + FS execution    |
+----------+-----------+   +-----------+-------------+
           |                           |
           +-------------+-------------+
                         v
                .guided/ filesystem
```

---

# 4. Stack Requirements

## 4.1 Programming Language

* Rust stable
* Edition 2021 or higher

## 4.2 Core Libraries

* **CLI**: `clap`
* **Async runtime**: `tokio`
* **Logging**: `tracing`, `tracing-subscriber`
* **HTTP**: `reqwest`
* **Serialization**: `serde`, `serde_json`
* **YAML**: `serde_yaml`
* **Templating**: `handlebars`
* **SQL**: `rusqlite` or `sqlx` (sync OK for SQLite)
* **FS utils**: `walkdir`

## 4.3 Patterns

* Workspace-centric architecture.
* Crate-per-domain separation.
* Prompt-as-code (structured YAML).
* Deterministic outputs.
* No hidden state.

---

# 5. Command Specifications & Flowcharts

# 5.1 `/ask` Command

**Purpose:** Ask an LLM a question using optional workspace + knowledge context.

### Flowchart

```
[User Input]
     |
     v
[Parse args]
     |
     v
[Load prompt definition (agent.ask.default)]
     |
     v
[Override context settings (--with-workspace, --knowledge-base)]
     |
     v
[Retrieve knowledge chunks (if requested)]
     |
     v
[Build final prompt (Handlebars + context injection)]
     |
     v
[Create LLM client via factory]
     |
     v
[Create LLM request from built prompt]
     |
     v
[Execute: streaming or non-streaming]
     |
     v
[Output to stdout (plain text or JSON)]
     |
     v
[Log metadata to stderr (if verbose)]
```

### Requirements

* Supports streaming (default) and non-streaming (`--no-stream`).
* Supports `--json` mode with structured output.
* Supports optional `--knowledge-base <name>` (stub in Phase 4, full RAG in Phase 5).
* Supports `--with-workspace` for file tree context injection.
* Supports `--file` for reading prompts from files.
* Supports `--max-tokens` and `--temperature` for generation control.
* Uses prompt builder exclusively.
* Never logs to stdout (only results).
* All logs and errors go to stderr.

### CLI Options

* `prompt` (positional) — Question text
* `--prompt` — Question text (explicit flag)
* `--file`, `-f` — Read prompt from file
* `--knowledge-base`, `-k` — Knowledge base name
* `--with-workspace` — Include workspace file tree
* `--stream` (default: true) — Enable streaming
* `--no-stream` — Disable streaming
* `--json` — Output as structured JSON
* `--max-tokens` — Maximum response tokens
* `--temperature` — Generation temperature (0.0-2.0)
* `--format`, `-o` — Output format

### JSON Output Format

```json
{
  "answer": "Response text",
  "model": "llama3",
  "provider": "ollama",
  "usage": {
    "promptTokens": 67,
    "completionTokens": 16,
    "totalTokens": 83
  },
  "metadata": {
    "promptId": "agent.ask.default",
    "workspaceContext": false,
    "knowledgeBase": null
  }
}
```

---

# 5.2 `/task` Command

**Purpose:** Plan and execute multi-step engineering tasks.

### Subcommands

* `task plan`
* `task run`
* `task show`

### Flowchart (plan)

```
[task plan]
   |
   v
[Load prompt.task.plan]
   |
   v
[Collect workspace summary]
   |
   v
[LLM -> TaskPlan]
   |
   v
[Save plan to .guided/tasks/<id>.json]
```

### Flowchart (run)

```
[task run]
    |
    v
[Load existing plan]
    |
    v
[For each step]
    |
    +--> [Apply edit / create file]
    |
    +--> [Validate]
    |
    +--> [Record log entry]
    |
    v
[Write results to stdout]
```

### Requirements

* Atomic file writes.
* Each step logged.
* Plan format JSON only.

---

# 5.3 `/knowledge` Command

**Purpose:** Local-first RAG using SQLite and embeddings.

### Subcommands

* `knowledge learn`
* `knowledge ask`
* `knowledge clean`
* `knowledge stats`

### Flowchart (learn)

```
[knowledge learn]
    |
    v
[Scan paths / fetch URLs]
    |
    v
[Parse -> raw text]
    |
    v
[Chunk]
    |
    v
[Embed]
    |
    v
[Insert vectors in SQLite]
    |
    v
[Update sources.jsonl + stats.json]
```

### Flowchart (ask)

```
[knowledge ask]
    |
    v
[Embed query]
    |
    v
[Top-k retrieval]
    |
    v
[Build prompt with context]
    |
    v
[LLM response]
```

### Requirements

* Base must always use the same embedding model.
* Retrieval must support cosine similarity.
* Parsing must support MD, HTML, PDF, source code.

---

# 5.4 `/stats` Command

**Purpose:** Provide local analytics.

### Flowchart

```
[stats]
   |
   v
[Load stats.json]
   |
   v
[Render JSON or table]
```

### Requirements

* Support `--json`.
* Tracks per-command counters.

---

# 6. Prompt System Specification

## 6.1 Prompt Definitions

Located at:

```
.grounded/prompts/*.yml
```

Fields:

* `id`
* `title`
* `apiVersion`
* `behavior` (tone/style)
* `input`
* `context`
* `template`
* `output`

## 6.2 Prompt Builder

Steps:

1. Load YAML.
2. Load workspace context (optional).
3. Load knowledge chunks (optional).
4. Render template (handlebars).
5. Produce final `system` + `user` payload for the provider.

---

# 7. Error Handling Specification

* All functions return `Result<T, AppError>`.
* Error categories:

  * Config errors
  * IO errors
  * LLM API errors
  * Knowledge index errors
  * Prompt build errors
  * Task execution errors

---

# 8. Testing Requirements

* Unit tests for each crate.
* Snapshot tests for prompt builder.
* SQLite tests for knowledge base.
* Integration tests for CLI commands.
* Streaming tests with mock LLM provider.

---

# 9. Output Specification

## stdout

* Prompt answers
* JSON outputs
* Task results

## stderr

* tracing logs
* errors
* debug output

---

# 10. Future Extensions

* HTTP server mode
* Incremental knowledge update
* Multiple parallel knowledge bases
* Agent-to-agent communication

---

This SPEC document defines the core architecture, command flows, non-functional requirements, and engineering constraints necessary to implement the Guided Agent CLI safely and predictably.
