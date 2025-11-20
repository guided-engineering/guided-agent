# Phase 5.5.1: Improve RAG Accuracy with Ollama Embeddings and Rich Metadata

**Version:** 1.0  
**Created:** 2025-11-20  
**Status:** Design → Implementation  
**Depends on:** Phase 5.5 (RAG Orchestrator), Phase 5.4 (Embeddings), Phase 5.3 (Chunking), Phase 5.2 (LanceDB)

## Executive Summary

This phase improves RAG accuracy by:
1. **Implementing Ollama embeddings** (`nomic-embed-text`) for better semantic matching
2. **Enriching metadata** with file type, language, tags, timestamps
3. **Indexing metadata as structured fields** in LanceDB for filtering
4. **Using filters to improve retrieval** quality and relevance

## Current State Analysis

### What Works (Phase 5.5)

✅ **Trigram embeddings** - Local, fast, deterministic  
✅ **LanceDB vector search** - Cosine similarity with IVF-PQ indexing  
✅ **RAG answering** - LLM synthesis with source references  
✅ **Basic metadata** - `source_path`, `file_size_bytes`, `file_line_count`  
✅ **Source tracking** - `sources.jsonl` with JSONL append  

### Accuracy Issues Identified

❌ **Weak semantic matching**:
- Trigram embeddings have score ~0.08-0.15 (threshold had to be lowered)
- Query "what is slugify?" gets score 0.0999 (barely passes)
- Abstract queries fail: "qual arquivo é maior?" retrieves wrong chunks

❌ **Missing metadata**:
- No file type classification (markdown vs code vs docs)
- No language detection (PT-BR vs EN vs programming language)
- No tags or categories for filtering
- No content hash for deduplication

❌ **No structured filtering**:
- Can't filter by file type or language
- Can't prioritize recent documents
- Can't rank by relevance metadata

❌ **LLM hallucination**:
- With weak retrieval, LLM invents information
- Example: Asked about file sizes, mentioned non-existent files
- Needed to reduce temperature to 0.1 and strengthen prompts

### Performance Baseline

**Trigram Embeddings (current)**:
- Dimension: 384
- Score range: 0.08-0.20 typical
- Semantic accuracy: Low (character-based)
- Speed: <10ms per query
- Cost: Free (local)

**Test queries (my-code base)**:
| Query | Score | Retrieved | Correct? |
|-------|-------|-----------|----------|
| "what is slugify?" | 0.0999 | 1 chunk | ✅ Yes |
| "o que é slug" | 0.159 | 1 chunk | ✅ Yes |
| "qual arquivo é maior?" | 0.159 | 1 chunk | ❌ Wrong (needs both files) |
| "compare file sizes" | 0.15-0.20 | 2 chunks | ✅ Yes |

## Solution Design

### 1. Ollama Embeddings (nomic-embed-text)

**Why Ollama + nomic-embed-text?**

- **Semantic quality**: Neural embeddings understand meaning, not just characters
- **Local-first**: Ollama runs locally (no API costs, privacy-preserving)
- **Performance**: 768-dim embeddings, state-of-art semantic matching
- **Multilingual**: Supports PT-BR, EN, and 100+ languages
- **Fast**: ~50ms per batch with GPU acceleration

**API Reference**:
```bash
# Ollama Embeddings API
POST http://localhost:11434/api/embeddings
{
  "model": "nomic-embed-text",
  "prompt": "text to embed"
}

Response:
{
  "embedding": [0.123, -0.456, ...] # 768 dimensions
}
```

**Configuration**:
```yaml
# .guided/knowledge/<base>/config.yaml
name: my-code
provider: ollama          # NEW: ollama embedding provider
model: nomic-embed-text   # NEW: 768-dim semantic embeddings
chunk_size: 512
chunk_overlap: 64
max_context_tokens: 2048
embedding_dim: 768        # CHANGED: from 384 (trigram) to 768 (nomic)
```

**Expected Improvements**:
| Metric | Trigram | Ollama (expected) |
|--------|---------|-------------------|
| Score range | 0.08-0.20 | 0.30-0.80 |
| Semantic accuracy | Low | High |
| Multilingual | Limited | Excellent |
| Query abstraction | Poor | Good |
| Threshold | 0.08 | 0.30 |

### 2. Rich Metadata Model

**Extended ChunkMetadata**:
```rust
pub struct ChunkMetadata {
    // Content type
    pub content_type: ContentType,      // text, markdown, code, html, pdf
    pub file_type: FileType,             // NEW: more specific classification
    pub language: Option<Language>,      // programming language or natural language
    
    // File identity
    pub source_path: String,             // full path
    pub file_name: String,               // NEW: filename only
    pub file_size_bytes: u64,            // already implemented
    pub file_line_count: usize,          // already implemented
    pub file_modified_at: DateTime<Utc>,// NEW: file modification timestamp
    
    // Chunk position
    pub byte_range: (usize, usize),
    pub line_range: Option<(usize, usize)>,
    pub char_count: usize,
    pub token_count: Option<usize>,
    
    // Content analysis
    pub content_hash: String,            // NEW: SHA-256 for deduplication
    pub tags: Vec<String>,               // NEW: ["api", "doc", "guide"]
    
    // Timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,       // NEW: chunk update timestamp
    
    // Splitter info
    pub splitter_used: String,
}
```

**FileType Classification**:
```rust
pub enum FileType {
    Markdown,
    Html,
    Pdf,
    Code(String),      // "rust", "typescript", "python"
    Text,
    Json,
    Yaml,
    Unknown,
}
```

**Language Detection**:
```rust
pub enum Language {
    // Natural languages
    Portuguese,
    English,
    Spanish,
    
    // Programming languages
    Rust,
    TypeScript,
    JavaScript,
    Python,
    Go,
    
    Unknown,
}
```

**Automatic Derivation**:
- `file_type` → from extension (.md, .rs, .ts, .html)
- `language` → heuristic (programming lang from extension, natural lang from content)
- `tags` → from path conventions (`docs/api/` → ["docs", "api"])
- `content_hash` → SHA-256 of chunk text
- `timestamps` → from file system + indexing time

### 3. LanceDB Schema with Metadata Columns

**Current Schema** (Phase 5.2/5.5):
```rust
struct ChunkRecord {
    id: String,
    source_id: String,
    text: String,
    embedding: Vec<f32>,  // 384-dim (trigram)
    metadata: Value,      // JSON blob
}
```

**New Schema** (Phase 5.5.1):
```rust
struct ChunkRecord {
    // Identity
    id: String,
    source_id: String,
    
    // Content
    text: String,
    embedding: Vec<f32>,  // 768-dim (nomic-embed-text)
    
    // Structured metadata (indexed fields)
    source_path: String,
    file_name: String,
    file_type: String,         // NEW
    language: String,          // NEW
    tags: Vec<String>,         // NEW (LanceDB list type)
    
    // File metadata
    file_size_bytes: u64,
    file_line_count: u64,
    file_modified_at: i64,     // NEW: Unix timestamp
    
    // Chunk metadata
    byte_range_start: u64,
    byte_range_end: u64,
    char_count: u64,
    content_hash: String,      // NEW
    
    // Timestamps
    created_at: i64,           // NEW: Unix timestamp
    updated_at: i64,           // NEW: Unix timestamp
    
    // Additional metadata (JSON blob for extensibility)
    metadata_extra: Value,
}
```

**LanceDB Type Mapping**:
| Rust Type | LanceDB Type | Notes |
|-----------|--------------|-------|
| String | Utf8 | Indexed for filtering |
| u64 | UInt64 | Numeric comparisons |
| i64 | Int64 | Timestamps (Unix epoch) |
| Vec<f32> | FixedSizeList<Float32> | Vector embeddings |
| Vec<String> | List<Utf8> | String arrays (tags) |
| Value | Struct | JSON-like nested data |

**Benefits**:
✅ Fast filtering by `file_type`, `language`, `tags`  
✅ Range queries on `created_at`, `file_modified_at`  
✅ Deduplication via `content_hash`  
✅ Efficient sorting by relevance metadata  

### 4. Filtered Search for RAG

**Search API**:
```rust
pub struct SearchOptions {
    pub query: String,
    pub top_k: usize,
    pub filters: Option<SearchFilters>,
}

pub struct SearchFilters {
    pub file_types: Option<Vec<String>>,     // ["markdown", "code"]
    pub languages: Option<Vec<String>>,      // ["rust", "english"]
    pub tags: Option<Vec<String>>,           // ["api", "guide"]
    pub created_after: Option<DateTime<Utc>>,
    pub min_score: Option<f32>,
}

pub async fn search(
    index: &LanceDBIndex,
    embedding: &[f32],
    options: &SearchOptions,
) -> AppResult<Vec<(KnowledgeChunk, f32)>> {
    // 1. Vector search with pre-filter
    // 2. Apply metadata filters
    // 3. Post-rank by metadata
    // 4. Return top-k with scores
}
```

**Filter Examples**:

```rust
// Filter by file type
let filters = SearchFilters {
    file_types: Some(vec!["markdown".to_string()]),
    ..Default::default()
};

// Filter by language (natural language)
let filters = SearchFilters {
    languages: Some(vec!["portuguese".to_string()]),
    ..Default::default()
};

// Filter by tags
let filters = SearchFilters {
    tags: Some(vec!["api".to_string(), "docs".to_string()]),
    ..Default::default()
};

// Recent documents only
let filters = SearchFilters {
    created_after: Some(Utc::now() - Duration::days(30)),
    ..Default::default()
};
```

**Default Filter Strategy** (when no filters specified):
1. Detect query language (PT-BR vs EN) → prefer same language docs
2. If query mentions code → prefer `file_type: code`
3. Prefer more recent documents (boost by `created_at`)
4. Down-rank generated/log files

### 5. Integration with RAG Orchestrator

**Updated ask_rag() Flow**:
```
User query
  ↓
1. Detect query language/intent
  ↓
2. Apply default filters if none specified
  ↓
3. Embed query with Ollama/nomic-embed-text
  ↓
4. Vector search with metadata filters
  ↓
5. Retrieve top-k chunks (higher scores ~0.5+)
  ↓
6. Build context with metadata
  ↓
7. LLM synthesis with stronger prompts
  ↓
8. Return answer + sources
```

**CLI Remains Unchanged**:
```bash
# Same UX as Phase 5.5
guided knowledge ask my-code "qual arquivo é maior?"

# Internally now uses:
# - Ollama embeddings (better semantic matching)
# - Metadata filters (language, file type)
# - Structured fields (file_size_bytes directly available)
```

## Before vs After Examples

### Example 1: Abstract Query

**Query**: "qual arquivo é maior?" (which file is larger?)

**BEFORE** (Trigram + minimal metadata):
```
Retrieved: 1 chunk (score: 0.159)
- index.js (byte offset 47-166)

LLM Response: "I could not find this information..."
```

**AFTER** (Ollama + rich metadata):
```
Retrieved: 2 chunks (scores: 0.65, 0.58)
- filter.js (450 bytes, 13 lines)
- index.js (178 bytes, 9 lines)

LLM Response: "O arquivo filter.js é maior com 450 bytes 
(13 linhas), comparado a index.js com 178 bytes (9 linhas)."
```

### Example 2: Multilingual Query

**Query**: "como funciona o slugify?" (PT-BR)

**BEFORE**:
```
Retrieved: 1 chunk (score: 0.12)
Wrong chunk retrieved (low semantic match)
```

**AFTER**:
```
Retrieved: 1 chunk (score: 0.72)
Correct chunk with slugify function
Language filter: prefer PT-BR docs if available
```

### Example 3: Code-Specific Query

**Query**: "show me all TypeScript utility functions"

**BEFORE**:
```
Retrieved: mixed results (markdown, JS, TS)
Filtering needed manual inspection
```

**AFTER**:
```
Filter: file_type = "code" AND language = "typescript"
Retrieved: only .ts files with utility functions
Tags: ["utils", "helpers"]
```

## Implementation Plan

### Phase 1: Ollama Embeddings
- [ ] Create `crates/knowledge/src/embeddings/providers/ollama.rs`
- [ ] Implement `OllamaProvider` with HTTP client (reqwest)
- [ ] Add retry logic and timeout handling
- [ ] Integrate with `EmbeddingEngine::create_provider()`
- [ ] Add tests with mock HTTP responses
- [ ] Update config schema to support `provider: ollama`

### Phase 2: Rich Metadata
- [ ] Create `crates/knowledge/src/metadata/mod.rs`
- [ ] Implement `FileType` enum and detection logic
- [ ] Implement `Language` enum and detection heuristics
- [ ] Implement tag derivation from paths
- [ ] Add content hashing (SHA-256)
- [ ] Update `ChunkMetadata` struct with all fields
- [ ] Integrate metadata generation into chunking pipeline

### Phase 3: LanceDB Schema Update
- [ ] Create `crates/knowledge/src/index/schema.rs`
- [ ] Define new `ChunkRecord` struct with all metadata fields
- [ ] Implement schema migration logic
- [ ] Update `LanceDBIndex` to use new schema
- [ ] Add tests for schema creation and querying

### Phase 4: Filtered Search
- [ ] Create `crates/knowledge/src/rag/search.rs`
- [ ] Implement `SearchFilters` struct
- [ ] Implement filtered vector search
- [ ] Add default filter strategy
- [ ] Integrate with `ask_rag()`
- [ ] Add comprehensive tests

### Phase 5: Validation
- [ ] Reindex `my-code` base with new pipeline
- [ ] Run test queries and measure improvements
- [ ] Document score improvements (0.08-0.15 → 0.50-0.80)
- [ ] Validate metadata indexing
- [ ] Test filtered queries
- [ ] Update architecture docs

## Success Metrics

| Metric | Current (Trigram) | Target (Ollama) | Actual |
|--------|-------------------|-----------------|--------|
| Average score | 0.12 | 0.50 | TBD |
| Min threshold | 0.08 | 0.30 | TBD |
| Query recall | 60% | 90% | TBD |
| Hallucination rate | High | Low | TBD |
| Filter coverage | 0% | 80% | TBD |

## Migration Strategy

**Backward Compatibility**:
- Keep `trigram` provider for existing bases
- Ollama is opt-in via config update
- Old bases continue working with old schema
- New bases use new schema by default

**Migration Path**:
```bash
# Option 1: Reindex with new provider
guided knowledge clean my-base
# Edit .guided/knowledge/my-base/config.yaml:
#   provider: ollama
#   model: nomic-embed-text
#   embedding_dim: 768
guided knowledge learn my-base --path <sources>

# Option 2: Keep trigram, add metadata only
# Update schema, reindex without changing embeddings
```

## API Stability

✅ **No CLI changes** - all improvements are internal  
✅ **Same command structure** - `guided knowledge ask/learn/clean/stats`  
✅ **Same output format** - natural language answers + sources  
✅ **Config-driven** - provider choice in `.guided/knowledge/<base>/config.yaml`  

## References

- [Ollama API Documentation](https://github.com/ollama/ollama/blob/main/docs/api.md)
- [nomic-embed-text Model](https://ollama.com/library/nomic-embed-text)
- [LanceDB Schema Documentation](https://lancedb.github.io/lancedb/)
- Phase 5.5: RAG Orchestrator
- Phase 5.4: Embedding Engine
- Phase 5.3: Chunking Pipeline
- Phase 5.2: LanceDB Integration

## Next Steps

After Phase 5.5.1:
- **Phase 5.6**: Advanced ranking (BM25 + semantic fusion)
- **Phase 5.7**: Query expansion and reformulation
- **Phase 5.8**: Incremental updates and change detection
- **Phase 5.9**: Multi-modal embeddings (code + docs)
