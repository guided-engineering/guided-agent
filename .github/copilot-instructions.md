# Guided Agent CLI — AI Coding Instructions

## Project Overview

A Rust-based CLI for AI-assisted development with local-first RAG (knowledge bases), multi-step task planning/execution, and LLM interaction. Architecture is **workspace-centric** with all state stored in `.guided/` directory.

## Core Architecture

```
CLI (clap) → Core (config, errors) → {LLM Providers, Knowledge Base (RAG), Prompt Builder, Task Engine} → .guided/ filesystem
```

**Crate separation by domain:**
- Core: `AppConfig`, `AppError`, logging
- LLM: trait-based provider abstraction (`LLMClient`, `LlmRequest`/`LlmResponse`)
- Knowledge: SQLite-backed RAG with embeddings (`KnowledgeChunk`, vector search)
- Prompt: YAML-based prompt definitions in `.guided/prompts/*.yml` rendered via Handlebars
- Task: Planning and file-system execution (`TaskPlan`, `TaskStep`, `TaskStepAction`)

## Critical Conventions

### Error Handling
- **All functions return `Result<T, AppError>`** with unified error enum covering Config, IO, LLM, Knowledge, Prompt, Task categories
- **Never panic** — errors must be represented and propagated

### I/O & Output
- **stdout**: LLM responses, JSON outputs, task results only
- **stderr**: `tracing` logs, errors, debug info
- **Atomic file writes**: use write → fsync → rename pattern
- **JSON outputs**: support `--json` flag on all commands for structured data

### CLI Text Communication
- **No emojis** — keep all CLI output plain text
- **Objective communication** — direct, concise messages without unnecessary explanations
- **No decorative elements** — avoid headers, banners, boxes, or ASCII art in command output

### Performance Targets
- CLI init: <120ms
- Streaming LLM: <300ms to first token
- Knowledge retrieval (top-k): <150ms for 50k chunks

## Filesystem Contract

All state lives in `.guided/`:

```
.guided/
├── prompts/              # PromptDefinition YAML files
├── tasks/                # TaskPlan JSON + execution logs
│   ├── <task-id>.json
│   └── <task-id>.log.json
├── knowledge/            # Per-base SQLite indexes
│   └── <base>/
│       ├── config.yaml   # KnowledgeBaseConfig
│       ├── index.sqlite  # Embeddings + chunks
│       ├── sources.jsonl # KnowledgeSource entries
│       └── stats.json    # BaseStats
└── operation/
    └── stats.json        # UsageStats
```

## Commands & Data Flow

### `ask` — LLM Query with Context
1. Parse args → Load prompt definition (`.guided/prompts/*.yml`)
2. Resolve workspace context (optional)
3. Retrieve knowledge chunks (optional, if `--knowledge-base` specified)
4. Build final prompt via Handlebars template
5. Stream LLM response to stdout

**Key entities:** `AskCommand`, `PromptDefinition`, `BuiltPrompt`, `LlmRequest`

### `task` — Multi-Step Planning & Execution
**Subcommands:** `plan`, `run`, `show`

**Plan flow:**
1. Collect workspace summary
2. LLM generates structured `TaskPlan` (JSON with steps)
3. Save to `.guided/tasks/<id>.json`

**Run flow:**
1. Load plan from JSON
2. Execute each `TaskStep` (create/edit/delete files)
3. Log results to `.guided/tasks/<id>.log.json`

**Key entities:** `TaskPlan`, `TaskStep`, `TaskStepAction` (enum: `CreateFile`, `EditFile`, `DeleteFile`, `RunCommand`), `TaskExecutionResult`

### `knowledge` — Local RAG
**Subcommands:** `learn`, `ask`, `clean`, `stats`

**Learn flow:**
1. Scan paths/URLs → parse text (MD, HTML, PDF, code)
2. Chunk text → generate embeddings
3. Insert vectors into SQLite with cosine similarity support
4. Update `sources.jsonl` and `stats.json`

**Ask flow:**
1. Embed query → top-k retrieval
2. Build context-enriched prompt → LLM response

**Critical:** Same embedding model must be used consistently per base.

**Key entities:** `KnowledgeBaseConfig`, `KnowledgeChunk`, `LearnOptions`, `AskOptions`

## Prompt System

Prompts are **structured YAML files** in `.guided/prompts/`:

```yaml
id: agent.ask.default
title: "Default Ask Prompt"
apiVersion: "1.0"
behavior:
  tone: professional
  style: concise
context:
  includeWorkspaceContext: true
  includeKnowledgeBase: false
template: |
  {{#if workspaceContext}}
  Workspace: {{workspaceContext}}
  {{/if}}
  Question: {{prompt}}
output:
  format: markdown
```

**Rendering:** Handlebars with context injection → produces `BuiltPrompt` (system + user strings for LLM)

## Development Workflow

- **Stack:** Rust stable (2021+), `clap`, `tokio`, `tracing`, `reqwest`, `serde`, `handlebars`, `rusqlite`/`sqlx`, `walkdir`
- **Testing:** Unit tests per crate, snapshot tests for prompts, SQLite integration tests, streaming mock tests
- **Observability:** All commands emit spans: `command.start/end`, `llm.request`, `knowledge.learn/retrieve`, `task.plan/run`

## Security & Portability

- **Network calls only to:** LLM provider, user-specified URLs in `knowledge learn`
- **No workspace content sent** unless explicitly triggered
- **Platforms:** macOS (Intel/ARM), Linux (x86_64/ARM)

## Key Design Principles

1. **Deterministic outputs** — no hidden state
2. **Prompt-as-code** — structured YAML, version-controlled
3. **Workspace-centric** — all context derives from `.guided/`
4. **Crate-per-domain** — clear boundaries between LLM/knowledge/task/prompt subsystems
5. **Atomic operations** — file writes are safe and transactional

## Common Patterns

- Global options (`--workspace`, `--provider`, `--model`) resolve to `AppConfig` at startup
- All commands support `--json` for machine-readable output
- Streaming is default for interactive commands (toggle with `--stream`/`--no-stream`)
- Use `tracing` for all logging (never `println!` for diagnostics)

## References

- Full specs: `docs/1-SPEC.md`
- Entity definitions: `docs/2-ENTITIES.md`
- Command dictionary: `docs/3-DICTIONARY.md`
