# Phase 0 – Guided Agent CLI Initialization

**Status:** ✓ Complete  
**Date:** 2025-11-19  
**Version:** 1.0.0

## Overview

This document describes the foundational setup for the Guided Agent CLI monorepo. Phase 0 establishes the minimal correct architecture required for all further development, including workspace structure, crate organization, and core utilities.

## Architecture

```
guided-agent/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── core/               # Core library (guided-core)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs      # Module exports
│   │       ├── error.rs    # AppError & AppResult
│   │       ├── logging.rs  # Tracing setup
│   │       └── config.rs   # AppConfig loader
│   └── cli/                # CLI binary (guided)
│       ├── Cargo.toml
│       └── src/
│           └── main.rs     # Entry point + Clap parser
└── .guided/                # Workspace state directory
    └── architecture/       # Documentation
        └── phase0.setup.md # This file
```

## Crates

### `guided-core`

**Purpose:** Foundational library providing shared utilities for all subsystems.

**Modules:**
- `error`: Unified error handling (`AppError` enum, `AppResult<T>` alias)
- `logging`: Tracing subscriber initialization (stderr output)
- `config`: Configuration loader with environment variable support

**Key Traits:**
- All functions return `Result<T, AppError>`
- Never panics — errors are represented and propagated
- Workspace-centric design

### `guided` (CLI binary)

**Purpose:** Command-line interface and command routing.

**Structure:**
- Clap-based argument parsing
- Global options: `--workspace`, `--provider`, `--model`
- Commands: `ask`, `task`, `knowledge`, `stats`
- Command handlers return `AppResult<()>`

**Commands (stubs):**
- `ask`: LLM query with optional context
  - Options: `--knowledge-base`, `--format`, `--json`
- `task plan/run/show`: Multi-step task planning and execution
- `knowledge learn/ask/clean/stats`: Local RAG management
- `stats`: Usage statistics

## Files Created

### Root Configuration

**`Cargo.toml`**
- Workspace definition with resolver 2
- Shared workspace dependencies
- Member crates: `crates/core`, `crates/cli`

### Core Crate

**`crates/core/Cargo.toml`**
- Package name: `guided-core`
- Dependencies: `anyhow`, `thiserror`, `tracing`, `tracing-subscriber`, `serde`, `serde_json`, `serde_yaml`

**`crates/core/src/lib.rs`**
- Module exports and re-exports
- Public API: `AppConfig`, `AppError`, `AppResult`

**`crates/core/src/error.rs`**
- `AppError` enum with variants:
  - `Config`, `Io`, `Llm`, `Knowledge`, `Prompt`, `Task`, `Serialization`, `Other`
- From implementations for `std::io::Error`, `serde_json::Error`, `serde_yaml::Error`
- `AppResult<T>` type alias

**`crates/core/src/logging.rs`**
- `init_logging()` function
- Stderr-only output (stdout reserved for data)
- Environment-based filtering via `RUST_LOG`
- Human-readable format

**`crates/core/src/config.rs`**
- `AppConfig` struct with fields:
  - `workspace`: PathBuf
  - `provider`: String
  - `model`: String
  - `api_key`: Option<String>
  - `log_level`: Option<String>
- `load()` method with environment variable support:
  - `GUIDED_WORKSPACE`, `GUIDED_PROVIDER`, `GUIDED_MODEL`, `GUIDED_API_KEY`, `RUST_LOG`
- `ensure_guided_dir()` method for `.guided/` creation

### CLI Crate

**`crates/cli/Cargo.toml`**
- Package name: `guided`
- Dependencies: `guided-core`, `clap`, `tokio`, `anyhow`, `tracing`

**`crates/cli/src/main.rs`**
- Clap parser with global options
- Tokio async runtime
- Command routing to stub handlers
- All handlers return `AppResult<()>`
- Commands:
  - `ask <prompt>`: Query LLM
  - `task plan <description>`: Create task plan
  - `task run <task-id>`: Execute task
  - `task show <task-id>`: Display task
  - `knowledge learn <sources> --base <name>`: Ingest sources
  - `knowledge ask <query> --base <name>`: Query knowledge
  - `knowledge clean --base <name>`: Clean knowledge base
  - `knowledge stats [--base <name>]`: Show statistics
  - `stats`: Show usage statistics

## Key Design Decisions

### Error Handling
- **Unified `AppError` enum** covering all subsystems
- **Never panic** — all errors propagated via `Result`
- **Category-based variants** for clear error origins

### I/O Conventions
- **stdout**: LLM responses, JSON outputs, command results only
- **stderr**: Tracing logs, errors, debug info
- **No decorative output**: Plain text only, no emojis or ASCII art

### Configuration
- **Environment-first** with CLI flag overrides
- **Workspace-centric** — all state in `.guided/`
- **Portable** across platforms

### Logging
- **Tracing** for structured observability
- **Stderr output** to avoid polluting data streams
- **Span-based** for command lifecycle tracking

## Workspace Dependencies

Core dependencies used across the workspace:

- **Async:** `tokio`, `futures`
- **CLI:** `clap` with derive features
- **Logging:** `tracing`, `tracing-subscriber`
- **Serialization:** `serde`, `serde_json`, `serde_yaml`
- **Errors:** `anyhow`, `thiserror`
- **HTTP:** `reqwest` (prepared for future LLM integration)
- **Templating:** `handlebars` (prepared for prompt system)
- **Database:** `rusqlite` (prepared for knowledge base)
- **Filesystem:** `walkdir` (prepared for workspace scanning)

## Build Verification

All commands completed successfully:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features
cargo build --workspace
```

### Build Status
- **Format:** ✓ Pass
- **Lint:** ✓ Pass (zero warnings)
- **Build:** ✓ Success

## Next Steps

Phase 0 provides the foundation for:

1. **Phase 1:** LLM provider integration (trait-based client abstraction)
2. **Phase 2:** Prompt system (YAML definitions + Handlebars templates)
3. **Phase 3:** Knowledge base (SQLite + vector embeddings)
4. **Phase 4:** Task engine (planning + file-system execution)

## Testing

Basic module structure verified with:
- Unit tests in `config.rs` and `logging.rs`
- Compilation success across all crates
- CLI help output verification

## Performance Targets

Phase 0 establishes baseline for future optimization:
- CLI init: <120ms (to be measured)
- Config load: <10ms (to be measured)
- Logging init: <5ms (to be measured)

## Security

- No network calls in Phase 0
- No external dependencies with known vulnerabilities
- Safe file operations with proper error handling
- No workspace content exposed

## Compatibility

- **Rust:** 1.91.1 (stable 2021 edition)
- **Platforms:** macOS (Intel/ARM), Linux (x86_64/ARM)
- **Shell:** zsh (primary), bash (compatible)

## Conclusion

Phase 0 successfully establishes the minimal correct foundation for the Guided Agent CLI. All core utilities are in place, the workspace structure is defined, and the build system is verified. The codebase is ready for feature development in subsequent phases.
