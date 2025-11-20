# Quiet Mode Architecture

**Status:** 🚧 In Progress  
**Date:** 2025-11-20  
**Phase:** 5.3.1

## Overview

This document describes the implementation of quiet mode (`-q` / `--quiet`) for the Guided Agent CLI, allowing users to suppress all diagnostic logs and see only clean LLM output on stdout.

## Current State Analysis

### Logging Architecture

**Current Setup:**
- `tracing` subscriber writes to stderr
- Log level configurable via `RUST_LOG` env var or `--log-level` flag
- `--verbose` flag sets debug level
- Logs include: info, debug, warn, error levels

**Output Paths:**
1. **Logs → stderr** (via tracing)
   - Command lifecycle (start/end)
   - Operation progress (learn, ask, task)
   - LLM request/response metadata
   - Debug information

2. **User Output → stdout** (via println!)
   - LLM responses (streaming or complete)
   - JSON output (--json mode)
   - Command results (stats, knowledge info)

**Current Issues:**
- Some commands mix logs with user output
- No way to completely silence stderr logs
- JSON output can be polluted with log messages
- No distinction between "quiet" and "no logs"

### Commands Using println!/eprintln!

**AskCommand:**
- Line 202: `println!("{}", json);` - JSON output
- Streaming: Direct stdout write for LLM tokens
- Non-JSON: Pretty-printed response

**KnowledgeCommand:**
- Lines 85-87: Learn results (JSON or human)
- Lines 140-148: Ask results with chunk display
- Line 170: Clean confirmation
- Lines 201-208: Stats display (JSON or human)

**TaskCommand:**
- Lines 63-64, 117-118, 146-147: Placeholder messages

**StatsCommand:**
- Lines 42-43: Placeholder messages

### Desired Behavior

## Behavior Matrix

| Mode                    | stdout                          | stderr                     | Streaming              |
|-------------------------|---------------------------------|----------------------------|------------------------|
| **Default**             | LLM response + results          | All tracing logs           | Token-by-token         |
| **--quiet**             | LLM response + results ONLY     | Errors only                | Token-by-token         |
| **--json**              | Valid JSON output               | All tracing logs           | Complete JSON at end   |
| **--json --quiet**      | Valid JSON output ONLY          | Errors only (critical)     | Complete JSON at end   |

### Output Mode Rules

1. **Default Mode (no flags)**
   - stdout: Command output (LLM responses, results)
   - stderr: All tracing logs (info, debug, warn, error)
   - Behavior: Maximum visibility for debugging

2. **Quiet Mode (--quiet)**
   - stdout: Command output ONLY (clean LLM responses)
   - stderr: Critical errors only (ERROR level)
   - Behavior: Clean output for scripting/piping
   - Log level: Automatically set to ERROR

3. **JSON Mode (--json)**
   - stdout: Valid JSON ONLY (no extra text)
   - stderr: All tracing logs (unchanged)
   - Behavior: Machine-readable output
   - Format: Single JSON object (not streaming)

4. **JSON + Quiet Mode (--json --quiet)**
   - stdout: Valid JSON ONLY (absolutely no extras)
   - stderr: Critical errors only (ERROR level)
   - Behavior: Pure JSON for automation
   - Error handling: Emit JSON error envelope on failure

## Design

### CLI Flag

```rust
/// Quiet mode: suppress logs, show only output
#[arg(short = 'q', long, global = true)]
quiet: bool,
```

**Properties:**
- Short: `-q`
- Long: `--quiet`
- Global: Available for all commands
- Semantics: "Suppress diagnostic logs, show only command output"

### Output Abstraction

**New Module:** `crates/cli/src/output.rs`

```rust
/// CLI output mode configuration
#[derive(Debug, Clone, Copy)]
pub struct OutputConfig {
    pub quiet: bool,
    pub json: bool,
    pub no_color: bool,
}

impl OutputConfig {
    /// Determine effective log level
    pub fn log_level(&self) -> &'static str {
        if self.quiet {
            "error"  // Only show critical errors
        } else {
            "info"   // Default level
        }
    }
    
    /// Check if logs should be suppressed
    pub fn suppress_logs(&self) -> bool {
        self.quiet
    }
    
    /// Check if output should be JSON
    pub fn is_json(&self) -> bool {
        self.json
    }
}

/// Output writer for command results
pub struct OutputWriter {
    config: OutputConfig,
}

impl OutputWriter {
    pub fn new(config: OutputConfig) -> Self {
        Self { config }
    }
    
    /// Write LLM response (respects quiet/json modes)
    pub fn write_response(&self, text: &str) -> AppResult<()> {
        if self.config.json {
            self.write_json_response(text)?;
        } else {
            println!("{}", text);
        }
        Ok(())
    }
    
    /// Write JSON output (validates format)
    pub fn write_json<T: Serialize>(&self, data: &T) -> AppResult<()> {
        let json = serde_json::to_string_pretty(data)?;
        println!("{}", json);
        Ok(())
    }
    
    /// Write error (respects quiet mode)
    pub fn write_error(&self, error: &str) {
        if self.config.json {
            // Emit JSON error envelope
            let error_obj = json!({
                "error": error,
                "success": false
            });
            eprintln!("{}", serde_json::to_string_pretty(&error_obj).unwrap());
        } else {
            eprintln!("Error: {}", error);
        }
    }
    
    /// Write streaming token (respects json mode)
    pub fn write_token(&self, token: &str) -> AppResult<()> {
        if !self.config.json {
            print!("{}", token);
            std::io::stdout().flush()?;
        }
        Ok(())
    }
}
```

### Integration Points

**1. Main CLI → Logging Init**

```rust
// main.rs
let output_config = OutputConfig {
    quiet: cli.quiet,
    json: cli.json,  // Assuming --json flag exists or will be added
    no_color: cli.no_color,
};

// Override log level if quiet
let effective_log_level = if cli.quiet {
    Some("error")
} else {
    cli.log_level.as_deref()
};

logging::init_logging(effective_log_level, cli.no_color)?;
```

**2. Command Handlers → Output Writer**

```rust
// ask.rs
let output = OutputWriter::new(output_config);

if stream {
    // Stream tokens
    while let Some(token) = stream.next().await {
        output.write_token(&token)?;
    }
} else {
    // Write complete response
    output.write_response(&response)?;
}
```

**3. JSON Output → Structured**

```rust
// knowledge.rs - learn command
if output.is_json() {
    output.write_json(&LearnOutput {
        sources: stats.sources_count,
        chunks: stats.chunks_count,
        bytes: stats.bytes_processed,
        duration_secs: stats.duration.as_secs_f64(),
    })?;
} else {
    println!("Learned {} sources ({} chunks, {} bytes) in {:.2}s",
        stats.sources_count,
        stats.chunks_count,
        stats.bytes_processed,
        stats.duration.as_secs_f64()
    );
}
```

## Streaming Behavior

### Non-JSON Mode (Default + Quiet)

**Default:**
- LLM tokens streamed to stdout immediately
- Logs appear on stderr simultaneously
- User sees: logs (stderr) + tokens (stdout)

**Quiet:**
- LLM tokens streamed to stdout immediately
- No logs on stderr (except errors)
- User sees: tokens (stdout) only

### JSON Mode (--json, --json --quiet)

**Challenge:** Streaming JSON is not standard JSON format.

**Decision:** 
- JSON mode disables streaming
- Collect full response, then emit one JSON object
- Alternative: Use NDJSON (newline-delimited JSON) if streaming required

**Implementation:**
```rust
if output.is_json() {
    // Collect all tokens
    let mut full_response = String::new();
    while let Some(token) = stream.next().await {
        full_response.push_str(&token);
    }
    // Emit once
    output.write_json(&AskOutput {
        response: full_response,
        success: true,
    })?;
} else {
    // Stream normally
    while let Some(token) = stream.next().await {
        output.write_token(&token)?;
    }
}
```

## Error Handling

### Non-JSON Mode

**Default:**
- Errors logged via tracing (ERROR level)
- Error message on stderr
- Exit code: non-zero

**Quiet:**
- Critical errors on stderr (minimal)
- Format: `Error: <message>`
- No stack traces or debug info
- Exit code: non-zero

### JSON Mode

**Option 1: JSON Error Envelope (chosen)**
```json
{
  "success": false,
  "error": "Failed to connect to LLM provider",
  "error_code": "LLM_CONNECTION_ERROR"
}
```

**Option 2: stderr + exit code**
- Emit human error to stderr
- Exit with non-zero code
- Automation checks exit code

**Decision:** Use Option 1 for consistency.

## Implementation Plan

### Phase 1: Core Infrastructure
- [x] Add `--quiet` flag to CLI
- [ ] Create `OutputConfig` struct
- [ ] Create `OutputWriter` abstraction
- [ ] Update logging init to respect quiet mode

### Phase 2: Command Migration
- [ ] Migrate `AskCommand` to use OutputWriter
- [ ] Migrate `KnowledgeCommand` to use OutputWriter
- [ ] Migrate `TaskCommand` (when implemented)
- [ ] Migrate `StatsCommand` (when implemented)

### Phase 3: JSON Mode Enhancement
- [ ] Add `--json` flag if not exists
- [ ] Implement JSON output for all commands
- [ ] Disable streaming in JSON mode
- [ ] Implement error envelopes

### Phase 4: Testing
- [ ] Unit tests for OutputConfig
- [ ] Unit tests for OutputWriter
- [ ] Integration tests for each mode combination
- [ ] Manual testing with real commands

## Testing Strategy

### Test Matrix

| Command           | Default | --quiet | --json | --json --quiet |
|-------------------|---------|---------|--------|----------------|
| ask (streaming)   | ✓       | ✓       | ✓      | ✓              |
| ask (no stream)   | ✓       | ✓       | ✓      | ✓              |
| knowledge learn   | ✓       | ✓       | ✓      | ✓              |
| knowledge ask     | ✓       | ✓       | ✓      | ✓              |
| knowledge stats   | ✓       | ✓       | ✓      | ✓              |
| Error cases       | ✓       | ✓       | ✓      | ✓              |

### Test Cases

**1. Default Mode**
```bash
guided ask "test question"
# Expected: logs on stderr + response on stdout
```

**2. Quiet Mode**
```bash
guided ask "test question" --quiet
# Expected: only response on stdout, no logs
```

**3. JSON Mode**
```bash
guided ask "test question" --json
# Expected: valid JSON on stdout + logs on stderr
```

**4. JSON + Quiet Mode**
```bash
guided ask "test question" --json --quiet
# Expected: only valid JSON on stdout, no logs
```

**5. Error in Quiet Mode**
```bash
guided ask "test" --quiet --provider invalid
# Expected: minimal error on stderr, exit code 1
```

**6. Error in JSON + Quiet Mode**
```bash
guided ask "test" --json --quiet --provider invalid
# Expected: JSON error envelope on stdout, exit code 1
```

## Limitations

1. **Streaming + JSON:** Not supported, buffered instead
2. **Partial JSON:** If process killed, no JSON emitted
3. **Quiet ≠ Silent:** Critical errors still shown
4. **No progress bars:** Quiet mode incompatible with interactive UI

## Future Enhancements

- **NDJSON Streaming:** Support `--json --stream` with newline-delimited JSON
- **Progress Tracking:** Silent progress mode (no logs, only progress bar)
- **Custom Formats:** `--format yaml|json|text`
- **Error Codes:** Structured error codes in JSON mode
- **Machine Mode:** `--machine` for full automation (no TTY, no colors, pure data)

## References

- Current logging: `crates/core/src/logging.rs`
- CLI parser: `crates/cli/src/main.rs`
- Ask command: `crates/cli/src/commands/ask.rs`
- Knowledge commands: `crates/cli/src/commands/knowledge.rs`
