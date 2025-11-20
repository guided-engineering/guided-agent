# Phase 5.5: RAG Orchestrator Architecture

**Version:** 1.0  
**Created:** 2025-11-20  
**Status:** Implementation

## Overview

Phase 5.5 consolidates the RAG (Retrieval-Augmented Generation) system by creating a unified orchestrator that coordinates:
- **Chunking** (Phase 5.3 hybrid pipeline)
- **Embeddings** (Phase 5.4 provider abstraction)
- **Vector indexing** (Phase 5.2 LanceDB)
- **LLM synthesis** (Phase 5.3.2 RAG answering)

The orchestrator provides a clean API for CLI commands while maintaining separation of concerns.

## Current State Analysis

### Existing Implementation (lib.rs)

**✅ Already implemented:**
- `learn(workspace, options, api_key)` - Learn flow with chunking + embedding + indexing
- `ask(workspace, options, api_key)` - Basic search without LLM synthesis
- `clean(workspace, base_name)` - Reset LanceDB index
- `stats(workspace, base_name)` - Get base statistics

**✅ RAG module (rag/):**
- `rag/ask.rs::ask_rag()` - Full RAG with LLM synthesis, natural language answers
- `rag/types.rs` - `RagResponse`, `RagSourceRef`, confidence thresholds

**🔧 Gaps to address:**
- No sources.jsonl persistence (source tracking)
- stats.json not persisted (last_learn_at, detailed metrics)
- URL and ZIP source support not implemented
- No orchestrator abstraction (logic scattered in lib.rs)
- `ask()` doesn't use `ask_rag()` - two separate implementations

## Architecture Design

### Module Structure

```
crates/knowledge/src/
├── rag/
│   ├── mod.rs           # Re-exports + RagOrchestrator struct
│   ├── orchestrator.rs  # Learn/ask/clean/stats orchestration
│   ├── ask.rs           # RAG answering with LLM synthesis (existing)
│   ├── types.rs         # RagResponse, RagStats (existing)
│   └── sources.rs       # Source tracking (sources.jsonl management) [NEW]
├── embeddings/          # Phase 5.4 - EmbeddingEngine
├── chunk/               # Phase 5.3 - Chunking pipeline
├── lancedb_index.rs     # Phase 5.2 - LanceDB integration
└── lib.rs               # Public API re-exports orchestrator functions
```

### RagOrchestrator Design

```rust
pub struct RagOrchestrator {
    workspace: PathBuf,
    base_name: String,
    config: KnowledgeBaseConfig,
}

impl RagOrchestrator {
    pub async fn new(workspace: &Path, base_name: &str) -> AppResult<Self>;
    
    // Core operations
    pub async fn learn(&mut self, options: &LearnOptions) -> AppResult<LearnStats>;
    pub async fn ask(&self, query: &str, options: &AskOptions) -> AppResult<RagResponse>;
    pub async fn clean(&mut self) -> AppResult<()>;
    pub async fn stats(&self) -> AppResult<RagStats>;
    
    // Internal helpers
    async fn process_sources(&mut self, sources: &[Source]) -> AppResult<LearnStats>;
    async fn track_source(&self, source: &KnowledgeSource) -> AppResult<()>;
}
```

## Flow Specifications

### 1. Learn Flow

**Sequence:**

```
CLI learn command
  ↓
RagOrchestrator::learn(options)
  ↓
1. Load/create config
  ↓
2. Initialize LanceDB index
  ↓
3. Reset if options.reset
  ↓
4. Discover sources (paths/URLs/ZIPs)
  ↓
5. For each source:
     a. Parse content (parser::parse_file/url/zip)
     b. Chunk via ChunkPipeline
     c. Embed via EmbeddingEngine
     d. Upsert to LanceDB
     e. Track in sources.jsonl
  ↓
6. Update stats.json
  ↓
7. Return LearnStats
```

**Data Flow:**

```
Source → Parser → ChunkPipeline → EmbeddingEngine → LanceDB
                                                      ↓
                                                  sources.jsonl
                                                  stats.json
```

**Implementation Notes:**
- Batch embeddings for performance (already in EmbeddingEngine)
- Atomic writes for sources.jsonl (append-only JSONL)
- Track: source_path, source_type, indexed_at, chunk_count, byte_count
- Support --reset to clear index before learning

### 2. Ask Flow

**Sequence:**

```
CLI ask command
  ↓
RagOrchestrator::ask(query, options)
  ↓
1. Load config
  ↓
2. Embed query via EmbeddingEngine
  ↓
3. Search LanceDB (top-k retrieval)
  ↓
4. Filter by MIN_RELEVANCE_SCORE (0.20 for mock, 0.3+ for production)
  ↓
5. Extract source metadata (source_path from custom field)
  ↓
6. Build context payload
  ↓
7. Call rag::ask::ask_rag() for LLM synthesis
  ↓
8. Return RagResponse
```

**RagResponse Structure:**

```rust
pub struct RagResponse {
    pub answer: String,
    pub sources: Vec<RagSourceRef>,
    pub max_score: f32,
    pub low_confidence: bool,
}

pub struct RagSourceRef {
    pub source: String,       // filename or "Unknown"
    pub location: String,     // "Lines X-Y" or "Position N"
    pub snippet: String,      // truncated to MAX_SNIPPET_LENGTH
}
```

**Implementation Notes:**
- Use existing `ask_rag()` from Phase 5.3.2
- Preserve metadata through entire pipeline
- Map UUIDs to human-readable source names
- Apply CONFIDENCE_THRESHOLD for low_confidence flag

### 3. Clean Flow

**Sequence:**

```
CLI clean command
  ↓
RagOrchestrator::clean()
  ↓
1. Load config
  ↓
2. Initialize LanceDB index
  ↓
3. Call index.reset()
  ↓
4. Delete sources.jsonl
  ↓
5. Delete stats.json
  ↓
6. Keep config.yaml (base configuration preserved)
```

**Implementation Notes:**
- Drop LanceDB table completely
- Preserve config.yaml (provider, model, dimensions)
- Allow re-learning with same configuration

### 4. Stats Flow

**Sequence:**

```
CLI stats command
  ↓
RagOrchestrator::stats()
  ↓
1. Load config
  ↓
2. Read index.stats() (sources_count, chunks_count)
  ↓
3. Calculate db_size_bytes (WalkDir)
  ↓
4. Read stats.json for last_learn_at
  ↓
5. Read sources.jsonl for source_list
  ↓
6. Return RagStats
```

**RagStats Structure:**

```rust
pub struct RagStats {
    pub base_name: String,
    pub sources_count: u32,
    pub chunks_count: u32,
    pub db_size_bytes: u64,
    pub last_learn_at: Option<chrono::DateTime<chrono::Utc>>,
    pub sources: Vec<SourceInfo>,
    pub embedding_provider: String,
    pub embedding_model: String,
}

pub struct SourceInfo {
    pub path: String,
    pub source_type: String,
    pub indexed_at: chrono::DateTime<chrono::Utc>,
    pub chunk_count: u32,
}
```

## LanceDB Integration

### Schema

**Table:** `chunks`

```rust
struct ChunkRecord {
    id: String,              // UUID
    source_id: String,       // UUID
    position: u32,           // Chunk position in source
    text: String,            // Chunk content
    vector: Vec<f32>,        // Embedding (384-dim for mock)
    metadata: Value,         // JSON with ChunkMetadata
}
```

**Metadata Fields:**
```json
{
  "content_type": "Code",
  "language": "Rust",
  "byte_range": [0, 1024],
  "line_range": [1, 50],
  "char_count": 1024,
  "hash": "abc123...",
  "created_at": "2025-11-20T...",
  "splitter_used": "CodeSplitter",
  "custom": {
    "source_path": "src/main.rs"
  }
}
```

### Indexing Strategy

**Vector Index:**
- Type: IVF-PQ (Inverted File Index with Product Quantization)
- Distance: Cosine similarity
- Configured via LanceDB automatically

**Metadata Filters:**
- Filter by `custom.source_path` for source-specific search
- Filter by `metadata.content_type` for code-only or text-only search
- Filter by `metadata.language` for language-specific search

## File System Layout

```
.guided/knowledge/<base>/
├── config.yaml           # Base configuration (provider, model, dimensions)
├── sources.jsonl         # Source tracking (append-only) [NEW]
├── stats.json            # Aggregated statistics [NEW]
└── index.lance/          # LanceDB directory
    ├── _latest.manifest
    ├── _versions/
    └── data/
```

### sources.jsonl Format

```jsonl
{"source_id":"uuid","path":"src/main.rs","type":"file","indexed_at":"2025-11-20T...","chunk_count":12,"byte_count":4096}
{"source_id":"uuid","path":"https://example.com/doc","type":"url","indexed_at":"2025-11-20T...","chunk_count":8,"byte_count":2048}
```

### stats.json Format

```json
{
  "base_name": "codebase",
  "last_learn_at": "2025-11-20T12:34:56Z",
  "total_sources": 25,
  "total_chunks": 1024,
  "total_bytes": 524288,
  "embedding_provider": "mock",
  "embedding_model": "trigram-v1"
}
```

## Implementation Plan

### Phase 5.5.1: Sources Module (NEW)

**File:** `crates/knowledge/src/rag/sources.rs`

```rust
pub struct SourceManager {
    workspace: PathBuf,
    base_name: String,
}

impl SourceManager {
    pub fn new(workspace: &Path, base_name: &str) -> Self;
    pub async fn track_source(&self, source: &KnowledgeSource) -> AppResult<()>;
    pub async fn list_sources(&self) -> AppResult<Vec<KnowledgeSource>>;
    pub async fn clear_sources(&self) -> AppResult<()>;
}
```

**Responsibilities:**
- Append to sources.jsonl (atomic writes)
- Read sources.jsonl (parse JSONL lines)
- Delete sources.jsonl (clean operation)

### Phase 5.5.2: Orchestrator Module (NEW)

**File:** `crates/knowledge/src/rag/orchestrator.rs`

Move learn/ask/clean/stats from `lib.rs` into orchestrator:

```rust
pub async fn learn(
    workspace: &Path,
    options: &LearnOptions,
    api_key: Option<&str>,
) -> AppResult<LearnStats> {
    // 1. Load config
    // 2. Initialize index
    // 3. Process sources (paths/URLs/ZIPs)
    // 4. Track in sources.jsonl
    // 5. Update stats.json
}

pub async fn ask(
    workspace: &Path,
    base_name: &str,
    query: &str,
    options: &AskOptions,
    api_key: Option<&str>,
) -> AppResult<RagResponse> {
    // Use existing rag::ask::ask_rag()
}

pub async fn clean(workspace: &Path, base_name: &str) -> AppResult<()> {
    // 1. Reset LanceDB
    // 2. Delete sources.jsonl
    // 3. Delete stats.json
}

pub async fn stats(workspace: &Path, base_name: &str) -> AppResult<RagStats> {
    // 1. Read index.stats()
    // 2. Read sources.jsonl
    // 3. Read stats.json
}
```

### Phase 5.5.3: Update lib.rs

**File:** `crates/knowledge/src/lib.rs`

```rust
// Re-export orchestrator functions
pub use rag::orchestrator::{learn, ask, clean, stats};

// Keep types
pub use rag::{RagResponse, RagSourceRef, RagStats};
pub use types::{AskOptions, LearnOptions, LearnStats, BaseStats};
```

**Goal:** Make `lib.rs` a thin re-export layer, moving logic to `rag/orchestrator.rs`.

### Phase 5.5.4: Update CLI

**File:** `crates/cli/src/commands/knowledge.rs`

No changes needed - CLI already uses `guided_knowledge::learn/ask/clean/stats` functions.

## Testing Strategy

### Unit Tests

**rag/sources.rs:**
- `test_track_source_creates_jsonl`
- `test_list_sources_parses_jsonl`
- `test_clear_sources_deletes_file`

**rag/orchestrator.rs:**
- `test_learn_creates_sources_jsonl`
- `test_learn_updates_stats_json`
- `test_ask_uses_rag_synthesis`
- `test_clean_removes_tracking_files`
- `test_stats_aggregates_metrics`

### Integration Tests

**tests/rag_flows.rs:**
```rust
#[tokio::test]
async fn test_full_learn_ask_clean_cycle() {
    // 1. Learn from multiple sources
    // 2. Ask questions and verify answers
    // 3. Verify sources.jsonl and stats.json
    // 4. Clean and verify everything removed
}
```

### Manual Testing

```bash
# Setup
cd /tmp/guided-test
mkdir -p docs
echo "Rust is a systems programming language" > docs/rust.md
echo "fn main() { println!(\"Hello\"); }" > docs/hello.rs

# Learn
guided knowledge learn testbase --path docs/ --reset

# Verify sources.jsonl created
cat .guided/knowledge/testbase/sources.jsonl

# Ask
guided knowledge ask testbase "What is Rust?"

# Stats
guided knowledge stats testbase --json

# Clean
guided knowledge clean testbase
```

## Migration Notes

### Backward Compatibility

**✅ No breaking changes:**
- Public API remains the same (`learn`, `ask`, `clean`, `stats`)
- CLI commands unchanged
- Config format unchanged

**🔧 Internal changes:**
- Logic moved from `lib.rs` to `rag/orchestrator.rs`
- Added sources.jsonl and stats.json persistence
- Enhanced ask() to use ask_rag() for LLM synthesis

### Upgrade Path

Existing bases will work without migration:
- sources.jsonl created on next learn
- stats.json created on next learn
- Missing files = empty arrays in stats

## Performance Considerations

### Embedding Batching

Already optimized in Phase 5.4:
- `EmbeddingEngine::embed_chunks()` batches internally
- Provider caching per base
- No re-computation of identical texts

### LanceDB Optimization

- IVF-PQ indexing for large datasets
- Configurable top-k limits
- In-memory caching for hot queries

### File I/O

- Atomic writes for sources.jsonl (append-only)
- Lazy loading of stats.json (only when needed)
- WalkDir for directory size calculation

## Limitations and Future Work

### Known Limitations

1. **URL sources:** Not implemented (parser::parse_url needed)
2. **ZIP sources:** Not implemented (parser::parse_zip needed)
3. **Incremental learning:** Always full rebuild (no diff-based updates)
4. **Re-ranking:** No semantic re-ranking beyond cosine similarity
5. **Diversity:** No diversity boosting for search results

### Future Enhancements (Post-Phase 5.5)

**Phase 5.6: Advanced RAG Features**
- Hybrid search (BM25 + vector)
- Re-ranking with cross-encoders
- Query expansion and reformulation
- Multi-hop reasoning

**Phase 5.7: Production Embeddings**
- OpenAI provider (text-embedding-3-small)
- Ollama provider (nomic-embed-text)
- Local GGUF models (llama.cpp integration)

**Phase 5.8: Incremental Learning**
- Diff-based chunk updates
- Source version tracking
- Efficient re-indexing

## References

### Internal Documentation
- Phase 5.2: `.guided/architecture/phase-5.2-lancedb.md`
- Phase 5.3: `.guided/architecture/phase-5.3-chunking.md`
- Phase 5.4: `.guided/architecture/phase-5.4-embeddings.md`

### External Resources
- LanceDB Rust API: https://docs.rs/lancedb/latest/lancedb/
- LanceDB Concepts: https://lancedb.github.io/lancedb/
- RAG Best Practices: https://www.pinecone.io/learn/retrieval-augmented-generation/

---

**Status:** Ready for implementation  
**Estimated effort:** 6-8 hours  
**Complexity:** Medium (refactoring + new modules)
