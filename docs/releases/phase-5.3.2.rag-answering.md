# Phase 5.3.2: RAG Answering UX Improvement

**Phase:** 5.3.2  
**Date:** 2025-11-20  
**Status:** In Progress

## Overview

Improve the user experience of `guided knowledge ask` to provide natural, human-readable answers instead of exposing internal RAG mechanics (chunks, embeddings, scores).

## Problem Statement

### Current State (Before Phase 5.3.2)

**Implementation:**
- `knowledge ask` only retrieves and displays raw chunks with scores
- No LLM integration for answer generation
- Output format exposes internal details:
  ```
  Retrieved 5 chunks:

  [1] Score: 0.856
  Gamedex é um aplicativo para gerenciar sua coleção de jogos...

  [2] Score: 0.743
  O app permite adicionar jogos de diferentes plataformas...
  ```

**Problems:**
1. **No actual answer** - Just raw context retrieval
2. **Exposes RAG internals** - Scores, chunk numbers, fragmented text
3. **Poor UX** - User must read multiple chunks to find their answer
4. **No LLM synthesis** - Missing the "answer generation" part of RAG

### Target State (After Phase 5.3.2)

**Clean output format:**
```
Answer:
Gamedex é um aplicativo multiplataforma para gerenciar coleções de jogos, permitindo catalogar jogos de diferentes plataformas como Steam, Epic Games, e consoles. O app oferece recursos de organização, busca e acompanhamento de progresso.

Sources:
- gamedex.md (lines 1-45)
- features.md (lines 12-28)
```

**Key improvements:**
- Natural language answer synthesized by LLM
- No mention of "chunks", "embeddings", "context", "vector search"
- Human-readable source references (file + location)
- Clear separation between answer and evidence
- Internal scores/IDs hidden (logged via tracing only)

---

## Architecture

### Type System

#### Public Types (User-Facing)

**File:** `crates/knowledge/src/rag/types.rs`

```rust
/// A single source reference used to answer a query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagSourceRef {
    /// Source file or document name (e.g., "gamedex.md", "playstore.html")
    pub source: String,
    
    /// Human-readable location within the source (e.g., "lines 12-34", "developer section")
    pub location: String,
    
    /// Short snippet showing the relevant evidence
    pub snippet: String,
}

/// Response from a RAG answering query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagResponse {
    /// Natural language answer synthesized by the LLM
    pub answer: String,
    
    /// List of sources used to generate the answer
    pub sources: Vec<RagSourceRef>,
    
    /// Internal: highest similarity score (for logging/debugging)
    #[serde(skip_serializing)]
    pub max_score: f32,
    
    /// Internal: whether confidence is low
    #[serde(skip_serializing)]
    pub low_confidence: bool,
}
```

#### Internal Types (Keep Existing)

**File:** `crates/knowledge/src/types.rs`

Existing types remain unchanged for internal operations:
- `KnowledgeChunk` - with `id`, `source_id`, `position`, `embedding`, `metadata`
- `AskResult` - with `chunks` and `scores` (now internal-only)

---

### RAG Orchestration Flow

**File:** `crates/knowledge/src/rag/ask.rs`

```rust
pub async fn ask_rag(
    workspace: &Path,
    options: AskOptions,
    api_key: Option<&str>,
) -> AppResult<RagResponse>
```

**Steps:**

1. **Retrieve chunks** (existing logic from `lib.rs::ask`)
   - Generate query embedding
   - Vector search with top-k
   - Apply `MIN_RELEVANCE_SCORE` threshold

2. **Check confidence**
   ```rust
   let max_score = scores.first().copied().unwrap_or(0.0);
   let low_confidence = max_score < CONFIDENCE_THRESHOLD;
   ```
   - `CONFIDENCE_THRESHOLD = 0.30` (configurable)
   - Low scores trigger cautious answering

3. **Build context for LLM**
   ```rust
   let context = chunks.iter()
       .zip(scores.iter())
       .map(|(chunk, _score)| format_chunk_context(chunk))
       .collect::<Vec<_>>()
       .join("\n\n---\n\n");
   ```

4. **Load RAG answering prompt**
   - Template: `.guided/prompts/agent.ask.rag.yml`
   - Variables: `{{query}}`, `{{context}}`, `{{low_confidence}}`

5. **Generate answer via LLM**
   - Send prompt + context to LLM
   - Stream or buffer response

6. **Map chunks to RagSourceRef**
   ```rust
   fn map_chunk_to_source_ref(chunk: &KnowledgeChunk) -> RagSourceRef {
       // Extract metadata
       let metadata: ChunkMetadata = serde_json::from_value(chunk.metadata.clone()).ok();
       
       RagSourceRef {
           source: extract_filename(&chunk.source_id),
           location: format_location(&metadata),
           snippet: truncate_snippet(&chunk.text, 150),
       }
   }
   ```

7. **Return RagResponse**

---

### CLI Output Format

**File:** `crates/cli/src/commands/knowledge/ask.rs`

#### Default Mode

```rust
if self.json {
    // JSON output
    let output = serde_json::to_json_pretty(&response)?;
    println!("{}", output);
} else {
    // Human-readable output
    println!("Answer:");
    println!("{}", response.answer);
    println!();
    
    if response.sources.is_empty() {
        println!("Sources: (no sources available)");
    } else {
        println!("Sources:");
        for source_ref in &response.sources {
            println!("- {} ({})", source_ref.source, source_ref.location);
        }
    }
}
```

#### JSON Mode

```json
{
  "answer": "Gamedex é um aplicativo...",
  "sources": [
    {
      "source": "gamedex.md",
      "location": "lines 1-45",
      "snippet": "Gamedex é um aplicativo para gerenciar..."
    }
  ]
}
```

**Rules:**
- No emojis, banners, or ASCII art
- One blank line between "Answer:" and "Sources:"
- All diagnostic info (scores, chunk IDs) goes to `tracing::debug!`
- Errors go to stderr

---

### System Prompt for RAG Answering

**File:** `.guided/prompts/agent.ask.rag.yml`

```yaml
id: agent.ask.rag
title: "RAG Answering System Prompt"
apiVersion: "1.0"
persona: "KnowledgeAssistant"

behavior:
  tone: professional
  style: direct
  constraints:
    - Do not mention "chunks", "embeddings", "vectors", "RAG", "context", "provided information"
    - Do not use phrases like "Based on the provided information" or "According to the context"
    - Answer as if you had read the original documents directly
    - If the context does not clearly state a fact, express uncertainty appropriately
    - Do not invent information not present in the context

uncertainty_handling:
  low_confidence: |
    When the context suggests something but does not explicitly state it, qualify your answer:
    Example: "The documents mention X in connection with Y, but do not explicitly confirm Z."
  
  insufficient_context: |
    When there is not enough information to answer, say:
    "I could not find this information in the available documents."

template: |
  You are a knowledge assistant with access to the user's document collection.
  
  {{#if low_confidence}}
  Note: The retrieved information may not directly answer this question. Be cautious and clear about what the documents do and do not state.
  {{/if}}
  
  User question:
  {{query}}
  
  Relevant context from documents:
  {{context}}
  
  Instructions:
  - Provide a clear, direct answer based only on the context above
  - Do not mention technical terms like "chunks", "embeddings", "context", or "RAG"
  - If the context suggests but does not confirm something, express that nuance
  - If the context does not contain the answer, state this clearly
  - Keep your response concise and factual

output:
  format: markdown
  max_length: 1000
```

---

## Implementation Details

### Similarity Thresholds

```rust
/// Minimum score for a chunk to be considered relevant
const MIN_RELEVANCE_SCORE: f32 = 0.20;

/// Minimum score for high-confidence answering
const CONFIDENCE_THRESHOLD: f32 = 0.30;
```

**Behavior:**
- `score < 0.20`: Chunk filtered out
- `0.20 ≤ score < 0.30`: Low confidence mode (cautious answering)
- `score ≥ 0.30`: Normal confidence (direct answering)

### Source Location Formatting

```rust
fn format_location(metadata: &ChunkMetadata) -> String {
    if let Some(line_range) = &metadata.line_range {
        format!("lines {}-{}", line_range.start, line_range.end)
    } else if let Some(byte_range) = &metadata.byte_range {
        format!("byte offset {}-{}", byte_range.start, byte_range.end)
    } else {
        "unknown location".to_string()
    }
}
```

### Snippet Truncation

```rust
fn truncate_snippet(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}...", &text[..max_len])
    }
}
```

---

## Before vs After Examples

### Example 1: Gamedex Creator Question

**Query:** "Quem criou o Gamedex?"

#### Before (Current)

```
Retrieved 5 chunks:

[1] Score: 0.683
Gamedex é um aplicativo multiplataforma para gerenciar sua coleção de jogos...

[2] Score: 0.612
O app foi desenvolvido por Adilson para ajudar jogadores...

[3] Score: 0.547
Contato: adilson@example.com
```

**Problems:**
- No synthesized answer
- User must read multiple chunks
- Unclear which chunk actually answers the question
- Scores are meaningless to users

#### After (Phase 5.3.2)

```
Answer:
O aplicativo foi desenvolvido por Adilson, conforme mencionado nos documentos. No entanto, os documentos não fornecem informações detalhadas sobre o histórico ou contexto da criação.

Sources:
- gamedex.md (lines 8-15)
- contact.md (lines 1-5)
```

**Improvements:**
- Clear, direct answer
- Acknowledges limitations ("não fornecem informações detalhadas")
- Shows which files were used
- No RAG jargon

---

### Example 2: Low Confidence Case

**Query:** "Qual é a data de lançamento do Gamedex?"

**Context:** Documents mention "lançado recentemente" but no specific date.

#### Output

```
Answer:
Os documentos mencionam que o Gamedex foi "lançado recentemente", mas não especificam uma data exata de lançamento.

Sources:
- gamedex.md (lines 1-10)
```

**Behavior:**
- `max_score = 0.25` (below CONFIDENCE_THRESHOLD)
- LLM receives `low_confidence: true` flag
- Answer acknowledges uncertainty
- No invented dates

---

### Example 3: Insufficient Information

**Query:** "Quantos downloads o Gamedex tem?"

**Context:** No documents mention download statistics.

#### Output

```
Answer:
I could not find this information in the available documents.

Sources: (no sources available)
```

**Behavior:**
- No chunks above relevance threshold
- Empty sources list
- Honest "no information" response

---

## Testing Strategy

### Unit Tests

**File:** `crates/knowledge/src/rag/tests.rs`

```rust
#[tokio::test]
async fn test_rag_response_with_sources() {
    // Test normal confidence case with multiple sources
}

#[tokio::test]
async fn test_rag_response_low_confidence() {
    // Test low confidence threshold behavior
}

#[tokio::test]
async fn test_rag_response_no_sources() {
    // Test when no chunks exceed relevance threshold
}

#[tokio::test]
async fn test_source_ref_location_formatting() {
    // Test line range, byte range, unknown location formats
}
```

### Integration Tests

**File:** `crates/cli/tests/knowledge_ask.rs`

```rust
#[tokio::test]
async fn test_knowledge_ask_output_format() {
    // Verify "Answer:" and "Sources:" blocks
    // Ensure no RAG jargon appears
}

#[tokio::test]
async fn test_knowledge_ask_json_output() {
    // Verify JSON structure matches RagResponse
}
```

### Manual Testing

1. **Learn Gamedex base**
   ```bash
   guided knowledge learn gamedex --path test-gamedex.md --reset
   ```

2. **Test normal confidence**
   ```bash
   guided knowledge ask gamedex "O que é o Gamedex?"
   ```

3. **Test low confidence**
   ```bash
   guided knowledge ask gamedex "Quem criou o Gamedex?"
   ```

4. **Test no information**
   ```bash
   guided knowledge ask gamedex "Quantos downloads tem?"
   ```

5. **Test JSON output**
   ```bash
   guided knowledge ask gamedex "O que é o Gamedex?" --json
   ```

---

## Configuration

### Per-Base Configuration (Future)

**File:** `.guided/knowledge/<base>/config.yaml`

```yaml
name: gamedex
provider: ollama
model: nomic-embed-text
embedding_dim: 384

# RAG answering settings
rag:
  min_relevance_score: 0.20
  confidence_threshold: 0.30
  max_sources: 5
  snippet_length: 150
```

---

## Migration Path

### Phase 1: Add RAG types and ask_rag function
- Create `rag/` module
- Implement `RagSourceRef` and `RagResponse`
- Implement `ask_rag()` with LLM integration
- Keep existing `ask()` function for backward compatibility

### Phase 2: Update CLI to use ask_rag
- Modify `KnowledgeAskCommand::execute()`
- Add new output format
- Preserve `--json` flag behavior

### Phase 3: Deprecate old ask()
- Add `#[deprecated]` annotation to `ask()`
- Update all callers to use `ask_rag()`
- Remove old function in next major version

---

## Open Questions

1. **LLM provider selection**
   - Use same provider as embeddings? Or allow override?
   - Decision: Use same provider, allow `--provider` override

2. **Prompt template location**
   - Store in `.guided/prompts/`? Or embed in code?
   - Decision: Store in `.guided/prompts/` for easy iteration

3. **Streaming support**
   - Should RAG answers stream or buffer?
   - Decision: Buffer answer, stream is opt-in via `--stream` flag

4. **Source deduplication**
   - Multiple chunks from same file/location - show once or multiple times?
   - Decision: Deduplicate by `(source, location)` tuple

---

## Success Criteria

- [ ] `knowledge ask` generates natural language answers
- [ ] No RAG jargon in user-facing output
- [ ] Sources shown in human-readable format (file + location)
- [ ] Low confidence cases handled appropriately
- [ ] All diagnostic info logged via tracing
- [ ] JSON output follows RagResponse schema
- [ ] Manual testing with Gamedex shows improved UX
- [ ] Before/after examples documented

---

## References

- Prompt schema: `.guided/schema/prompt.schema.json`
- Phase 5.3 (Chunking): `.guided/architecture/phase-5.3-hybrid-chunking.md`
- LLM integration: `crates/llm/src/lib.rs`
- Existing types: `crates/knowledge/src/types.rs`
