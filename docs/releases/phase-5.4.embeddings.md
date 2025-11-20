# Phase 5.4: Embedding Engine Refactoring

**Phase:** 5.4  
**Date:** 2025-11-20  
**Status:** In Progress

## Overview

Refactor the embedding layer to be provider-agnostic, deterministic per knowledge base, and tightly integrated with the Phase 5.3 chunking pipeline.

## Current State Analysis

### Problems Identified

1. **Scattered embedding logic**:
   - `lib.rs::generate_embedding()` - used by learn flow
   - `rag/ask.rs::generate_embedding()` - duplicated for RAG answering
   - Both use same mock implementation (trigram-based)
   - No abstraction for different providers

2. **Hardcoded provider/model**:
   - Config has `provider` and `model` fields but they're just strings
   - No validation that a base always uses same provider/model
   - No factory pattern for creating providers

3. **LLM client misuse**:
   - Currently using `guided_llm::create_client()` for embeddings
   - LLM clients are designed for text generation, not embeddings
   - Mock embedding doesn't use the client at all (`_client` parameter)

4. **No provider abstraction**:
   - Can't easily add OpenAI, Anthropic, local GGUF, etc.
   - No trait-based design for pluggability
   - Hard to test with different embedding models

5. **Configuration issues**:
   - `embedding_dim` stored in config but not validated
   - No way to enforce dimension consistency
   - Config doesn't capture all provider-specific details

### Current Call Sites

```rust
// lib.rs - learn flow
let client = create_client(&config.provider, None, api_key)?;
for chunk in chunks {
    let embedding = generate_embedding(client.as_ref(), &config.model, &chunk.text).await?;
    // store in LanceDB
}

// lib.rs - ask flow  
let client = create_client(&config.provider, None, api_key)?;
let query_embedding = generate_embedding(client.as_ref(), &config.model, &query).await?;
// search LanceDB

// rag/ask.rs - RAG answering
let embed_client = create_client(&config.provider, None, api_key)?;
let query_embedding = generate_embedding(embed_client.as_ref(), &config.model, &query).await?;
```

All use the same mock implementation that ignores `client` and `model` parameters.

---

## Target Architecture

### Design Principles

1. **Provider-agnostic**: Support multiple embedding providers through a trait
2. **Per-base determinism**: Each base must always use the same provider/model/dimensions
3. **Clean separation**: Embeddings module decoupled from LLM, HTTP, CLI concerns
4. **Configuration-driven**: Provider instantiation based on base config
5. **Testable**: Mock providers for unit tests, real providers for integration tests

### Module Structure

```
crates/knowledge/src/embeddings/
├── mod.rs           - Public API (EmbeddingEngine)
├── provider.rs      - EmbeddingProvider trait + factory
├── config.rs        - EmbeddingConfig types
├── providers/
│   ├── mod.rs
│   ├── mock.rs      - Mock provider (current trigram implementation)
│   ├── openai.rs    - OpenAI embeddings API
│   ├── ollama.rs    - Ollama embeddings API
│   └── gguf.rs      - Local GGUF embeddings (future)
└── tests.rs         - Integration tests
```

### Configuration Structure

**File:** `.guided/knowledge/<base>/config.yaml`

```yaml
name: gamedex
provider: mock  # or: openai, ollama, gguf
model: trigram-v1  # provider-specific model identifier

# Embedding configuration
embedding:
  dimensions: 384
  normalize: true
  batch_size: 100
  
  # Provider-specific settings
  provider_config:
    # For OpenAI:
    # api_base: https://api.openai.com/v1
    # org_id: org-xxx
    
    # For Ollama:
    # host: http://localhost:11434
    # timeout_secs: 30
    
    # For GGUF:
    # model_path: /path/to/model.gguf
    # threads: 4

# Chunking configuration (Phase 5.3)
chunk_size: 512
chunk_overlap: 64
max_context_tokens: 2048
```

### Core Types

#### EmbeddingProvider Trait

```rust
/// Trait for embedding providers.
#[async_trait::async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Get provider name (e.g., "openai", "ollama", "mock")
    fn provider_name(&self) -> &str;
    
    /// Get model identifier
    fn model_name(&self) -> &str;
    
    /// Get embedding dimensions
    fn dimensions(&self) -> usize;
    
    /// Generate embeddings for multiple texts in a batch.
    async fn embed_batch(&self, texts: &[String]) -> AppResult<Vec<Vec<f32>>>;
    
    /// Generate embedding for a single text (convenience method).
    async fn embed(&self, text: &str) -> AppResult<Vec<f32>> {
        let mut results = self.embed_batch(&[text.to_string()]).await?;
        results.pop().ok_or_else(|| AppError::Knowledge("No embedding returned".to_string()))
    }
}
```

#### EmbeddingEngine

```rust
/// Central embedding engine that manages providers per knowledge base.
pub struct EmbeddingEngine {
    workspace: PathBuf,
    providers: Arc<RwLock<HashMap<String, Arc<dyn EmbeddingProvider>>>>,
}

impl EmbeddingEngine {
    pub fn new(workspace: PathBuf) -> Self;
    
    /// Get or create provider for a knowledge base.
    async fn get_provider(&self, base_name: &str, api_key: Option<&str>) -> AppResult<Arc<dyn EmbeddingProvider>>;
    
    /// Embed multiple texts for a knowledge base.
    pub async fn embed_texts(&self, base_name: &str, texts: &[String], api_key: Option<&str>) -> AppResult<Vec<Vec<f32>>>;
    
    /// Embed chunks (extracts text from Chunk structs).
    pub async fn embed_chunks(&self, base_name: &str, chunks: &[Chunk], api_key: Option<&str>) -> AppResult<Vec<Vec<f32>>>;
    
    /// Validate that a base's config hasn't changed.
    fn validate_config_consistency(&self, base_name: &str, config: &EmbeddingConfig) -> AppResult<()>;
}
```

#### EmbeddingConfig

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    pub provider: String,
    pub model: String,
    pub dimensions: usize,
    pub normalize: bool,
    pub batch_size: usize,
    
    #[serde(default)]
    pub provider_config: serde_json::Value,
}

impl EmbeddingConfig {
    /// Load from base config.yaml
    pub fn load(workspace: &Path, base_name: &str) -> AppResult<Self>;
    
    /// Save to base config.yaml
    pub fn save(&self, workspace: &Path, base_name: &str) -> AppResult<()>;
    
    /// Validate config consistency (ensure same provider/model/dimensions)
    pub fn validate_consistency(&self, other: &Self) -> AppResult<()>;
}
```

---

## Provider Factory

```rust
/// Create an embedding provider based on config.
pub fn create_provider(config: &EmbeddingConfig, api_key: Option<&str>) -> AppResult<Arc<dyn EmbeddingProvider>> {
    match config.provider.as_str() {
        "mock" => Ok(Arc::new(MockProvider::new(config.dimensions))),
        
        "openai" => {
            let api_key = api_key.ok_or_else(|| AppError::Knowledge("OpenAI requires API key".to_string()))?;
            Ok(Arc::new(OpenAIProvider::new(
                &config.model,
                api_key,
                config.dimensions,
                config.provider_config.clone(),
            )?))
        }
        
        "ollama" => {
            let host = config.provider_config
                .get("host")
                .and_then(|v| v.as_str())
                .unwrap_or("http://localhost:11434");
            Ok(Arc::new(OllamaProvider::new(
                &config.model,
                host,
                config.dimensions,
            )?))
        }
        
        "gguf" => {
            let model_path = config.provider_config
                .get("model_path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AppError::Knowledge("GGUF provider requires model_path".to_string()))?;
            Ok(Arc::new(GgufProvider::new(
                model_path,
                config.dimensions,
            )?))
        }
        
        _ => Err(AppError::Knowledge(format!("Unknown embedding provider: {}", config.provider))),
    }
}
```

---

## Sequence Diagrams

### embed_texts Flow

```
┌──────┐                 ┌──────────────────┐                ┌────────────┐              ┌────────────────┐
│Client│                 │EmbeddingEngine   │                │Config      │              │Provider        │
└──┬───┘                 └────────┬─────────┘                └─────┬──────┘              └───────┬────────┘
   │                              │                                │                             │
   │ embed_texts(base, texts)     │                                │                             │
   │─────────────────────────────>│                                │                             │
   │                              │                                │                             │
   │                              │ get_provider(base)             │                             │
   │                              │───────────────────────────────>│                             │
   │                              │                                │                             │
   │                              │ load_config(base)              │                             │
   │                              │<───────────────────────────────│                             │
   │                              │                                │                             │
   │                              │          create_provider(config, api_key)                    │
   │                              │──────────────────────────────────────────────────────────────>│
   │                              │                                │                             │
   │                              │                                │     new Provider(config)    │
   │                              │                                │<────────────────────────────│
   │                              │                                │                             │
   │                              │ provider                       │                             │
   │                              │<───────────────────────────────────────────────────────────── │
   │                              │                                │                             │
   │                              │ embed_batch(texts)             │                             │
   │                              │──────────────────────────────────────────────────────────────>│
   │                              │                                │                             │
   │                              │                                │         [HTTP/Local call]   │
   │                              │                                │                             │
   │                              │ Vec<Vec<f32>>                  │                             │
   │                              │<──────────────────────────────────────────────────────────────│
   │                              │                                │                             │
   │ Vec<Vec<f32>>                │                                │                             │
   │<─────────────────────────────│                                │                             │
   │                              │                                │                             │
```

### embed_chunks Flow

```
┌──────┐                 ┌──────────────────┐                ┌────────────┐
│Learn │                 │EmbeddingEngine   │                │Provider    │
└──┬───┘                 └────────┬─────────┘                └─────┬──────┘
   │                              │                                │
   │ embed_chunks(base, chunks)   │                                │
   │─────────────────────────────>│                                │
   │                              │                                │
   │                              │ extract texts from chunks      │
   │                              │────┐                           │
   │                              │    │                           │
   │                              │<───┘                           │
   │                              │                                │
   │                              │ embed_batch(texts)             │
   │                              │───────────────────────────────>│
   │                              │                                │
   │                              │ Vec<Vec<f32>>                  │
   │                              │<───────────────────────────────│
   │                              │                                │
   │ Vec<Vec<f32>>                │                                │
   │<─────────────────────────────│                                │
   │                              │                                │
```

---

## Provider Implementations

### MockProvider (Current Trigram Implementation)

```rust
pub struct MockProvider {
    dimensions: usize,
}

impl MockProvider {
    pub fn new(dimensions: usize) -> Self {
        Self { dimensions }
    }
}

#[async_trait::async_trait]
impl EmbeddingProvider for MockProvider {
    fn provider_name(&self) -> &str {
        "mock"
    }
    
    fn model_name(&self) -> &str {
        "trigram-v1"
    }
    
    fn dimensions(&self) -> usize {
        self.dimensions
    }
    
    async fn embed_batch(&self, texts: &[String]) -> AppResult<Vec<Vec<f32>>> {
        // Use existing trigram-based mock implementation
        texts.iter()
            .map(|text| self.generate_mock_embedding(text))
            .collect()
    }
    
    fn generate_mock_embedding(&self, text: &str) -> AppResult<Vec<f32>> {
        // Copy existing logic from lib.rs::generate_embedding
    }
}
```

### OpenAIProvider

```rust
pub struct OpenAIProvider {
    model: String,
    api_key: String,
    dimensions: usize,
    client: reqwest::Client,
    api_base: String,
}

#[async_trait::async_trait]
impl EmbeddingProvider for OpenAIProvider {
    fn provider_name(&self) -> &str {
        "openai"
    }
    
    fn model_name(&self) -> &str {
        &self.model
    }
    
    fn dimensions(&self) -> usize {
        self.dimensions
    }
    
    async fn embed_batch(&self, texts: &[String]) -> AppResult<Vec<Vec<f32>>> {
        let url = format!("{}/embeddings", self.api_base);
        
        let request = serde_json::json!({
            "model": self.model,
            "input": texts,
        });
        
        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;
            return Err(AppError::Knowledge(format!(
                "OpenAI API error {}: {}",
                status, body
            )));
        }
        
        let result: OpenAIEmbeddingResponse = response.json().await?;
        
        Ok(result.data.into_iter().map(|item| item.embedding).collect())
    }
}
```

### OllamaProvider

```rust
pub struct OllamaProvider {
    model: String,
    host: String,
    dimensions: usize,
    client: reqwest::Client,
}

#[async_trait::async_trait]
impl EmbeddingProvider for OllamaProvider {
    // Similar to OpenAI but with Ollama API format
    // https://github.com/ollama/ollama/blob/main/docs/api.md#generate-embeddings
}
```

---

## Migration Strategy

### Phase 1: Create embeddings module
- Create `embeddings/` directory structure
- Implement `EmbeddingProvider` trait
- Implement `MockProvider` with existing logic
- Add unit tests

### Phase 2: Implement EmbeddingEngine
- Implement provider caching
- Implement config loading and validation
- Implement `embed_texts` and `embed_chunks` APIs

### Phase 3: Add real providers
- Implement `OpenAIProvider`
- Implement `OllamaProvider`
- Add integration tests

### Phase 4: Refactor call sites
- Replace `generate_embedding` in `lib.rs` with `EmbeddingEngine::embed_chunks`
- Replace `generate_embedding` in `rag/ask.rs` with `EmbeddingEngine::embed`
- Remove old `generate_embedding` functions
- Remove `guided_llm::create_client` usage for embeddings

### Phase 5: Update configuration
- Migrate existing bases to new config format
- Add validation on startup
- Document configuration in user guide

---

## Configuration Validation

### Rules

1. **First learn**: Config is created with provider/model/dimensions
2. **Subsequent learns**: Config must match exactly:
   - Same `provider`
   - Same `model`  
   - Same `dimensions`
3. **Mismatch**: Error with message:
   ```
   Embedding configuration mismatch for base 'gamedex':
     Expected: provider=mock, model=trigram-v1, dimensions=384
     Found: provider=openai, model=text-embedding-3-small, dimensions=1536
   
   To change embedding configuration, clean and recreate the base:
     guided knowledge clean gamedex
     guided knowledge learn gamedex --path ...
   ```

### Implementation

```rust
impl EmbeddingEngine {
    fn validate_config_consistency(&self, base_name: &str, new_config: &EmbeddingConfig) -> AppResult<()> {
        let index_path = config::get_index_path(&self.workspace, base_name);
        
        if !index_path.exists() {
            // New base, no validation needed
            return Ok(());
        }
        
        let existing_config = EmbeddingConfig::load(&self.workspace, base_name)?;
        
        if existing_config.provider != new_config.provider
            || existing_config.model != new_config.model
            || existing_config.dimensions != new_config.dimensions
        {
            return Err(AppError::Knowledge(format!(
                "Embedding configuration mismatch for base '{}':\n\
                 Expected: provider={}, model={}, dimensions={}\n\
                 Found: provider={}, model={}, dimensions={}\n\n\
                 To change embedding configuration, clean and recreate the base:\n\
                 guided knowledge clean {}\n\
                 guided knowledge learn {} --path ...",
                base_name,
                existing_config.provider, existing_config.model, existing_config.dimensions,
                new_config.provider, new_config.model, new_config.dimensions,
                base_name, base_name
            )));
        }
        
        Ok(())
    }
}
```

---

## Error Handling

### Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    #[error("Provider not found: {0}")]
    ProviderNotFound(String),
    
    #[error("API error: {0}")]
    ApiError(String),
    
    #[error("Configuration mismatch: {0}")]
    ConfigMismatch(String),
    
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
    
    #[error("Batch size exceeded: {0} > max {1}")]
    BatchSizeExceeded(usize, usize),
}
```

### Logging

All embedding operations logged via `tracing`:

```rust
tracing::info!(
    "Embedding {} texts for base '{}' using provider '{}' (model: {})",
    texts.len(),
    base_name,
    provider.provider_name(),
    provider.model_name()
);

tracing::debug!(
    "Generated embeddings: {} vectors of dimension {}",
    embeddings.len(),
    provider.dimensions()
);
```

---

## Testing Strategy

### Unit Tests

1. **Provider tests**:
   - Mock provider generates valid embeddings
   - Dimensions match config
   - Batch processing works

2. **Engine tests**:
   - Provider caching works
   - Config validation catches mismatches
   - Error handling for missing config

3. **Config tests**:
   - Load/save round-trip
   - Default values
   - Validation logic

### Integration Tests

1. **OpenAI provider** (requires API key):
   - Real API call
   - Rate limiting
   - Error handling

2. **Ollama provider** (requires local Ollama):
   - Local API call
   - Timeout handling
   - Model availability check

### Manual Testing

```bash
# Test with mock provider (default)
guided knowledge learn test --path file.md

# Test with OpenAI
echo "provider: openai\nmodel: text-embedding-3-small\nembedding:\n  dimensions: 1536" > .guided/knowledge/test/.../config.yaml
OPENAI_API_KEY=xxx guided knowledge learn test --path file.md

# Test config validation
guided knowledge learn test --path file.md  # Should error if provider changed
```

---

## Performance Considerations

1. **Batch processing**: Process texts in batches (default 100) to optimize API calls
2. **Provider caching**: Cache provider instances per base to avoid recreation
3. **Parallel requests**: For large batches, split into parallel requests
4. **Rate limiting**: Implement exponential backoff for API errors
5. **Local models**: Prefer local providers (Ollama, GGUF) for better latency

---

## Future Enhancements

1. **Additional providers**:
   - Anthropic/Claude embeddings
   - Google Gemini embeddings
   - Cohere embeddings
   - HuggingFace models

2. **Advanced features**:
   - Embedding caching (avoid re-embedding same text)
   - Fine-tuned models per base
   - Multi-vector embeddings
   - Dimension reduction (PCA, UMAP)

3. **Monitoring**:
   - Embedding quality metrics
   - API cost tracking
   - Performance profiling

---

## References

- **OpenAI Embeddings API**: https://platform.openai.com/docs/api-reference/embeddings
- **Ollama API**: https://github.com/ollama/ollama/blob/main/docs/api.md
- **LanceDB Vector Index**: https://lancedb.github.io/lancedb/
- **Phase 5.3 Chunking**: `.guided/architecture/phase-5.3-hybrid-chunking.md`
