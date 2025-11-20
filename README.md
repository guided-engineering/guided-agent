# Guided Agent CLI

AI-assisted development with local-first RAG (Retrieval-Augmented Generation).

## Features

- **Local-First**: Default Ollama provider for offline LLM access
- **Multi-Provider Support**: OpenAI, Anthropic Claude, Ollama, local GGUF models
- **Knowledge Base**: Build local RAG indexes from documentation and code
- **Task Planning**: Multi-step task execution with file operations
- **Streaming Support**: Real-time LLM responses
- **Workspace-Centric**: All state stored in `.guided/` directory

## Installation

```bash
cargo install --path crates/cli
```

## Quick Start

```bash
# Initialize workspace (creates .guided/ directory)
guided --workspace . ask "What is Rust?"

# Use Ollama (default, no API key required)
guided ask "Explain async/await"

# Stream responses (default)
guided ask "Write a simple HTTP server"

# Non-streaming mode with JSON output
guided ask "What is Cargo?" --no-stream --json
```

## Configuration

Configuration is managed through multiple layers with the following precedence:
1. **CLI flags** (highest priority)
2. **Environment variables**
3. **Config file** (`.guided/config.yaml`)
4. **Defaults** (lowest priority)

### Config File Structure

Create `.guided/config.yaml` in your workspace:

```yaml
llm:
  activeProvider: ollama
  activeEmbeddingProvider: ollama
  
  providers:
    openai:
      apiKeyEnv: OPENAI_API_KEY
      model: gpt-4
      embeddingModel: text-embedding-3-small
      endpoint: https://api.openai.com/v1
    
    claude:
      apiKeyEnv: ANTHROPIC_API_KEY
      model: claude-3-5-sonnet-20241022
      endpoint: https://api.anthropic.com/v1
      apiVersion: "2023-06-01"
    
    ollama:
      endpoint: http://localhost:11434
      model: llama3.2
      embeddingModel: nomic-embed-text
      timeout: 30
    
    gguf-local:
      modelPathEnv: GGUF_MODEL_PATH
      embeddingModelPathEnv: GGUF_EMBEDDING_MODEL_PATH
      threads: 4
      contextSize: 2048

workspace:
  path: "."

logging:
  level: info
  color: true
```

### Environment Variables

```bash
# Workspace
export GUIDED_WORKSPACE=/path/to/project
export GUIDED_CONFIG=/path/to/config.yaml

# LLM Provider
export GUIDED_PROVIDER=openai
export GUIDED_MODEL=gpt-4

# API Keys (referenced in config.yaml)
export OPENAI_API_KEY=sk-...
export ANTHROPIC_API_KEY=sk-ant-...

# GGUF Models
export GGUF_MODEL_PATH=/path/to/model.gguf
export GGUF_EMBEDDING_MODEL_PATH=/path/to/embed.gguf

# Logging
export RUST_LOG=debug
export NO_COLOR=1  # Disable colored output
```

### CLI Overrides

All config options can be overridden via CLI flags:

```bash
# Override provider and model
guided --provider openai --model gpt-4 ask "Hello"

# Override workspace
guided --workspace /path/to/project ask "What files exist?"

# Enable verbose logging
guided --verbose ask "Debug this"
guided --log-level debug ask "More details"

# Disable colors
guided --no-color ask "Plain text only"
```

## Commands

### `ask` - Query LLM

Ask questions with optional context from workspace or knowledge base.

```bash
# Simple query
guided ask "How does async work in Rust?"

# Read prompt from file
guided ask --file prompt.txt

# Query with knowledge base context
guided ask --knowledge-base rust-docs "What is a trait?"

# Control generation
guided ask "Write code" --max-tokens 500 --temperature 0.7

# Output formats
guided ask "Summarize" --format markdown
guided ask "Get JSON" --json
```

### `task` - Multi-Step Tasks

Plan and execute multi-step tasks with file operations.

```bash
# Plan a task
guided task plan "Create a REST API with authentication"

# Execute a plan
guided task run <task-id>

# Show task details
guided task show <task-id>
```

### `knowledge` - RAG Management

Build and query local knowledge bases.

```bash
# Learn from files
guided knowledge learn rust-docs ./docs/*.md

# Learn from URLs
guided knowledge learn web-docs https://example.com/docs

# Query knowledge base
guided knowledge ask rust-docs "What is borrowing?"

# Show statistics
guided knowledge stats rust-docs

# Clean unused data
guided knowledge clean rust-docs
```

### `stats` - Usage Statistics

View LLM usage and token consumption.

```bash
# Overall stats
guided stats

# By provider
guided stats --provider ollama

# Date range
guided stats --period last-week
```

## Providers

### Ollama (Default)

Local LLM runtime - no API key required.

```bash
# Install Ollama: https://ollama.ai
ollama pull llama3.2

# Use with Guided
guided ask "Hello"
```

### OpenAI

```bash
export OPENAI_API_KEY=sk-...
guided --provider openai --model gpt-4 ask "Hello"
```

### Anthropic Claude

```bash
export ANTHROPIC_API_KEY=sk-ant-...
guided --provider claude --model claude-3-5-sonnet-20241022 ask "Hello"
```

### GGUF Local Models

```bash
export GGUF_MODEL_PATH=/path/to/model.gguf
guided --provider gguf-local ask "Hello"
```

## Architecture

```
CLI (clap) → Core (config, errors) → {LLM Providers, Knowledge Base, Task Engine}
                                      ↓
                                  .guided/
                                    ├── config.yaml
                                    ├── prompts/
                                    ├── tasks/
                                    ├── knowledge/
                                    └── operation/
```

## Development

```bash
# Build all crates
cargo build

# Run tests
cargo test

# Check code quality
cargo clippy

# Format code
cargo fmt

# Run CLI locally
cargo run -- ask "Test"
```

## Project Structure

```
guided-agent/
├── crates/
│   ├── core/       # Error handling, config, logging
│   ├── llm/        # LLM abstraction and providers
│   └── cli/        # Command-line interface
├── docs/
│   ├── 0-PRD.md
│   ├── 1-SPEC.md
│   ├── 2-ENTITIES.md
│   ├── 3-DICTIONARY.md
│   └── 4-ROADMAP.md
└── .guided/        # Workspace state (created on first run)
```

## License

MIT OR Apache-2.0
