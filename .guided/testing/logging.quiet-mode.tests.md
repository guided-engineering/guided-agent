# Quiet Mode Testing Guide

**Phase:** 5.3.1  
**Date:** 2025-11-20

## Overview

This document describes test cases for the quiet mode (`--quiet` / `-q`) flag implementation.

## Test Matrix

| Command                | Mode          | Expected stdout                | Expected stderr        | Pass |
|------------------------|---------------|--------------------------------|------------------------|------|
| knowledge stats        | Default       | Stats output + newlines        | All INFO logs          | ✅   |
| knowledge stats        | --quiet       | Stats output only              | None (ERROR only)      | ✅   |
| knowledge learn        | Default       | Success message                | All INFO logs          | ✅   |
| knowledge learn        | --quiet       | Success message only           | None (ERROR only)      | ✅   |
| ask                    | Default       | LLM response                   | All INFO logs          | ⏳   |
| ask                    | --quiet       | LLM response only              | None (ERROR only)      | ⏳   |
| ask (streaming)        | Default       | Streamed tokens                | All INFO logs          | ⏳   |
| ask (streaming)        | --quiet       | Streamed tokens only           | None (ERROR only)      | ⏳   |
| knowledge clean        | Default       | Confirmation message           | All INFO logs          | ⏳   |
| knowledge clean        | --quiet       | Confirmation message only      | None (ERROR only)      | ⏳   |
| Error (invalid cmd)    | Default       | Error message                  | ERROR logs             | ⏳   |
| Error (invalid cmd)    | --quiet       | Error message                  | ERROR logs (minimal)   | ⏳   |

## Test Cases

### TC-1: Knowledge Stats - Default Mode

**Command:**
```bash
guided knowledge stats gamedex
```

**Expected Output:**
```
2025-11-20T11:25:01.389272Z  INFO guided: Guided Agent CLI starting
2025-11-20T11:25:01.389315Z  INFO command{name="knowledge"}: guided::commands::knowledge: Executing knowledge stats command for base 'gamedex'
2025-11-20T11:25:01.389320Z  INFO command{name="knowledge"}: guided_knowledge: Getting stats for knowledge base 'gamedex'
2025-11-20T11:25:01.390032Z  INFO command{name="knowledge"}:load: lance::dataset_events: event="loading" ...
Knowledge base: gamedex
  Sources: 0
  Chunks: 7
  DB size: 36661 bytes
2025-11-20T11:25:01.390398Z  INFO command{name="knowledge"}: guided: Command completed successfully
```

**Assertions:**
- stderr contains INFO logs
- stdout contains stats output
- Exit code: 0

**Status:** ✅ PASS

---

### TC-2: Knowledge Stats - Quiet Mode

**Command:**
```bash
guided knowledge stats gamedex --quiet
```

**Expected Output:**
```
Knowledge base: gamedex
  Sources: 0
  Chunks: 7
  DB size: 36661 bytes
```

**Assertions:**
- stderr is empty (no INFO logs)
- stdout contains only stats output
- Exit code: 0

**Status:** ✅ PASS

---

### TC-3: Knowledge Learn - Default Mode

**Command:**
```bash
guided knowledge learn gamedex --path test-gamedex.md
```

**Expected Output:**
```
2025-11-20T11:03:47.286849Z  INFO guided: Guided Agent CLI starting
2025-11-20T11:03:47.287175Z  INFO command{name="knowledge"}: guided::commands::knowledge: Executing knowledge learn command for base 'gamedex'
2025-11-20T11:03:47.287429Z  INFO command{name="knowledge"}: guided_knowledge: Starting learn operation for base 'gamedex'
[... LanceDB logs ...]
Learned 1 sources (7 chunks, 4622 bytes) in 0.04s
2025-11-20T11:03:47.343502Z  INFO command{name="knowledge"}: guided: Command completed successfully
```

**Assertions:**
- stderr contains INFO logs (including LanceDB verbose logs)
- stdout contains success message
- Exit code: 0

**Status:** ✅ PASS

---

### TC-4: Knowledge Learn - Quiet Mode

**Command:**
```bash
guided knowledge learn gamedex --path test-gamedex.md --quiet
```

**Expected Output:**
```
Learned 1 sources (7 chunks, 4622 bytes) in 0.07s
```

**Assertions:**
- stderr is empty (no INFO logs, no LanceDB logs)
- stdout contains only success message
- Exit code: 0

**Status:** ✅ PASS

---

### TC-5: Ask - Default Mode (Non-streaming)

**Command:**
```bash
guided ask "What is Rust?" --no-stream
```

**Expected Output:**
```
2025-11-20T11:30:00.000000Z  INFO guided: Guided Agent CLI starting
2025-11-20T11:30:00.000000Z  INFO command{name="ask"}: guided::commands::ask: Executing ask command
2025-11-20T11:30:00.000000Z  INFO command{name="ask"}: Sending non-streaming request to LLM
Rust is a systems programming language...
2025-11-20T11:30:05.000000Z  INFO command{name="ask"}: guided: Command completed successfully
```

**Assertions:**
- stderr contains INFO logs
- stdout contains LLM response
- Exit code: 0

**Status:** ⏳ TODO

---

### TC-6: Ask - Quiet Mode (Non-streaming)

**Command:**
```bash
guided ask "What is Rust?" --no-stream --quiet
```

**Expected Output:**
```
Rust is a systems programming language...
```

**Assertions:**
- stderr is empty
- stdout contains only LLM response
- Exit code: 0

**Status:** ⏳ TODO

---

### TC-7: Ask - Default Mode (Streaming)

**Command:**
```bash
guided ask "What is Rust?"
```

**Expected Output:**
```
2025-11-20T11:30:00.000000Z  INFO guided: Guided Agent CLI starting
2025-11-20T11:30:00.000000Z  INFO command{name="ask"}: guided::commands::ask: Executing ask command
2025-11-20T11:30:00.000000Z  INFO command{name="ask"}: Starting streaming request to LLM
Rust is a systems programming language... (streamed token by token)
2025-11-20T11:30:05.000000Z  INFO command{name="ask"}: guided: Command completed successfully
```

**Assertions:**
- stderr contains INFO logs
- stdout contains streamed LLM response
- Exit code: 0

**Status:** ⏳ TODO

---

### TC-8: Ask - Quiet Mode (Streaming)

**Command:**
```bash
guided ask "What is Rust?" --quiet
```

**Expected Output:**
```
Rust is a systems programming language... (streamed token by token)
```

**Assertions:**
- stderr is empty
- stdout contains only streamed LLM response
- No interleaved logs during streaming
- Exit code: 0

**Status:** ⏳ TODO

---

### TC-9: Error Case - Default Mode

**Command:**
```bash
guided ask "test" --provider invalid-provider
```

**Expected Output:**
```
2025-11-20T11:30:00.000000Z  INFO guided: Guided Agent CLI starting
2025-11-20T11:30:00.000000Z ERROR guided: Unknown provider: invalid-provider
Error: Unknown provider: invalid-provider
```

**Assertions:**
- stderr contains ERROR logs
- stderr or stdout contains error message
- Exit code: non-zero (1)

**Status:** ⏳ TODO

---

### TC-10: Error Case - Quiet Mode

**Command:**
```bash
guided ask "test" --provider invalid-provider --quiet
```

**Expected Output:**
```
Error: Unknown provider: invalid-provider
```

**Assertions:**
- stderr contains minimal error message (ERROR level only)
- No INFO/DEBUG/WARN logs
- Exit code: non-zero (1)

**Status:** ⏳ TODO

---

## Integration Test Script

```bash
#!/bin/bash
# Test quiet mode functionality

echo "=== TC-1: Knowledge Stats - Default ==="
guided knowledge stats gamedex 2>&1 | head -10

echo ""
echo "=== TC-2: Knowledge Stats - Quiet ==="
guided knowledge stats gamedex --quiet 2>&1

echo ""
echo "=== TC-3: Knowledge Learn - Default ==="
guided knowledge learn test-kb --path test-file.md 2>&1 | head -10

echo ""
echo "=== TC-4: Knowledge Learn - Quiet ==="
guided knowledge learn test-kb --path test-file.md --quiet 2>&1

echo ""
echo "=== TC-9: Error - Default ==="
guided ask "test" --provider invalid 2>&1

echo ""
echo "=== TC-10: Error - Quiet ==="
guided ask "test" --provider invalid --quiet 2>&1
```

## Manual Testing Steps

### Setup
1. Build release binary: `cargo build --release`
2. Ensure test knowledge base exists: `gamedex`
3. Ensure test file exists: `test-gamedex.md`

### Test Execution

**Test 1: Verify logs appear in default mode**
```bash
./target/release/guided knowledge stats gamedex 2>&1 | grep INFO
# Should output multiple INFO log lines
```

**Test 2: Verify logs suppressed in quiet mode**
```bash
./target/release/guided knowledge stats gamedex --quiet 2>&1 | grep INFO
# Should output nothing
```

**Test 3: Verify output still visible**
```bash
./target/release/guided knowledge stats gamedex --quiet
# Should show clean stats output
```

**Test 4: Verify error logs still shown**
```bash
./target/release/guided ask "test" --provider invalid --quiet 2>&1 | grep ERROR
# Should show ERROR level logs
```

## Automated Test Suite

### Unit Tests

**File:** `crates/core/src/logging.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_logging_default() {
        let result = init_logging(None, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_init_logging_error_level() {
        let result = init_logging(Some("error"), false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_init_logging_invalid_level() {
        let result = init_logging(Some("invalid"), false);
        assert!(result.is_err());
    }
}
```

### Integration Tests

**File:** `crates/cli/tests/quiet_mode.rs` (to be created)

```rust
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_quiet_mode_suppresses_logs() {
    let mut cmd = Command::cargo_bin("guided").unwrap();
    cmd.args(&["knowledge", "stats", "test-kb", "--quiet"]);
    
    cmd.assert()
        .success()
        .stderr(predicate::str::contains("INFO").not());
}

#[test]
fn test_default_mode_shows_logs() {
    let mut cmd = Command::cargo_bin("guided").unwrap();
    cmd.args(&["knowledge", "stats", "test-kb"]);
    
    cmd.assert()
        .success()
        .stderr(predicate::str::contains("INFO"));
}

#[test]
fn test_quiet_mode_shows_errors() {
    let mut cmd = Command::cargo_bin("guided").unwrap();
    cmd.args(&["ask", "test", "--provider", "invalid", "--quiet"]);
    
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("ERROR"));
}
```

## Performance Testing

### Scenario: Large file learn with quiet mode

**Objective:** Verify quiet mode doesn't impact performance.

**Test:**
```bash
# Baseline (default mode)
time guided knowledge learn perf-test --path large-file.md > /dev/null 2>&1

# Quiet mode
time guided knowledge learn perf-test --path large-file.md --quiet > /dev/null 2>&1
```

**Expected:** Similar execution times (quiet mode may be slightly faster due to less I/O).

## Known Issues

None currently.

## Future Enhancements

1. **JSON Output Mode** (`--json` flag)
   - Test pure JSON output
   - Test JSON + quiet mode
   - Verify no log pollution in JSON

2. **Streaming + JSON Mode**
   - Test buffered JSON collection
   - Verify single JSON object emitted

3. **Error JSON Envelopes**
   - Test error cases with --json --quiet
   - Verify structured error output

## References

- Architecture: `.guided/architecture/logging.quiet-mode.md`
- Worklog: `.guided/operation/logging.quiet-mode.worklog.md`
- Main CLI: `crates/cli/src/main.rs`
- Logging: `crates/core/src/logging.rs`
