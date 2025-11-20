//! LanceDB-backed vector index implementation.

use crate::types::KnowledgeChunk;
use crate::vector_index::VectorIndex;
use arrow_array::{FixedSizeListArray, RecordBatch, RecordBatchIterator, StringArray, UInt32Array};
use arrow_schema::{DataType, Field, Schema};
use guided_core::{AppError, AppResult};
use lancedb::query::{ExecutableQuery, QueryBase};
use lancedb::Table;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

/// LanceDB-backed vector index for knowledge chunks.
pub struct LanceDbIndex {
    table: Table,
    embedding_dim: usize,
    source_ids: HashSet<String>,
}

impl LanceDbIndex {
    /// Create or open a LanceDB index at the specified path.
    ///
    /// # Arguments
    /// * `db_path` - Directory path for the LanceDB database
    /// * `table_name` - Name of the table (typically "chunks")
    /// * `embedding_dim` - Dimension of embedding vectors (e.g., 384)
    pub async fn new(db_path: &Path, table_name: &str, embedding_dim: usize) -> AppResult<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::Knowledge(format!("Failed to create index directory: {}", e))
            })?;
        }

        // Connect to LanceDB
        let uri = db_path.to_string_lossy().to_string();
        let conn = lancedb::connect(&uri)
            .execute()
            .await
            .map_err(|e| AppError::Knowledge(format!("Failed to connect to LanceDB: {}", e)))?;

        // Check if table exists
        let table_names = conn
            .table_names()
            .execute()
            .await
            .map_err(|e| AppError::Knowledge(format!("Failed to list tables: {}", e)))?;

        let table = if table_names.contains(&table_name.to_string()) {
            // Open existing table
            conn.open_table(table_name)
                .execute()
                .await
                .map_err(|e| AppError::Knowledge(format!("Failed to open table: {}", e)))?
        } else {
            // Create new table with schema
            let schema = Self::create_schema(embedding_dim);
            let empty_batch = RecordBatch::new_empty(schema.clone());

            conn.create_table(
                table_name,
                RecordBatchIterator::new(vec![Ok(empty_batch)], schema),
            )
            .execute()
            .await
            .map_err(|e| AppError::Knowledge(format!("Failed to create table: {}", e)))?
        };

        tracing::debug!("Initialized LanceDB index at {:?}", db_path);

        Ok(Self {
            table,
            embedding_dim,
            source_ids: HashSet::new(),
        })
    }

    /// Create Arrow schema for chunks table.
    fn create_schema(embedding_dim: usize) -> Arc<Schema> {
        Arc::new(Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("source_id", DataType::Utf8, false),
            Field::new("position", DataType::UInt32, false),
            Field::new("text", DataType::Utf8, false),
            Field::new(
                "embedding",
                DataType::FixedSizeList(
                    Arc::new(Field::new("item", DataType::Float32, true)),
                    embedding_dim as i32,
                ),
                false,
            ),
            Field::new("metadata", DataType::Utf8, false),
        ]))
    }

    /// Convert KnowledgeChunk to Arrow RecordBatch.
    fn chunk_to_batch(&self, chunk: &KnowledgeChunk) -> AppResult<RecordBatch> {
        let schema = Self::create_schema(self.embedding_dim);

        let embedding = chunk
            .embedding
            .as_ref()
            .ok_or_else(|| AppError::Knowledge("Chunk missing embedding".to_string()))?;

        if embedding.len() != self.embedding_dim {
            return Err(AppError::Knowledge(format!(
                "Embedding dimension mismatch: expected {}, got {}",
                self.embedding_dim,
                embedding.len()
            )));
        }

        let metadata_json = serde_json::to_string(&chunk.metadata)
            .map_err(|e| AppError::Knowledge(format!("Failed to serialize metadata: {}", e)))?;

        // Create arrays
        let id_array = StringArray::from(vec![chunk.id.as_str()]);
        let source_id_array = StringArray::from(vec![chunk.source_id.as_str()]);
        let position_array = UInt32Array::from(vec![chunk.position]);
        let text_array = StringArray::from(vec![chunk.text.as_str()]);
        let metadata_array = StringArray::from(vec![metadata_json.as_str()]);

        // Create embedding as FixedSizeListArray
        let embedding_values = arrow_array::Float32Array::from(embedding.clone());
        let embedding_array = FixedSizeListArray::new(
            Arc::new(Field::new("item", DataType::Float32, true)),
            self.embedding_dim as i32,
            Arc::new(embedding_values),
            None,
        );

        RecordBatch::try_new(
            schema,
            vec![
                Arc::new(id_array),
                Arc::new(source_id_array),
                Arc::new(position_array),
                Arc::new(text_array),
                Arc::new(embedding_array),
                Arc::new(metadata_array),
            ],
        )
        .map_err(|e| AppError::Knowledge(format!("Failed to create RecordBatch: {}", e)))
    }

    /// Convert Arrow RecordBatch row to KnowledgeChunk.
    fn batch_to_chunk(&self, batch: &RecordBatch, row_idx: usize) -> AppResult<KnowledgeChunk> {
        let id = batch
            .column(0)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| AppError::Knowledge("Invalid id column".to_string()))?
            .value(row_idx)
            .to_string();

        let source_id = batch
            .column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| AppError::Knowledge("Invalid source_id column".to_string()))?
            .value(row_idx)
            .to_string();

        let position = batch
            .column(2)
            .as_any()
            .downcast_ref::<UInt32Array>()
            .ok_or_else(|| AppError::Knowledge("Invalid position column".to_string()))?
            .value(row_idx);

        let text = batch
            .column(3)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| AppError::Knowledge("Invalid text column".to_string()))?
            .value(row_idx)
            .to_string();

        let embedding_list = batch
            .column(4)
            .as_any()
            .downcast_ref::<FixedSizeListArray>()
            .ok_or_else(|| AppError::Knowledge("Invalid embedding column".to_string()))?;

        let embedding_array_ref = embedding_list.value(row_idx);
        let embedding_values = embedding_array_ref
            .as_any()
            .downcast_ref::<arrow_array::Float32Array>()
            .ok_or_else(|| AppError::Knowledge("Invalid embedding values".to_string()))?;

        let embedding: Vec<f32> = (0..embedding_values.len())
            .map(|i| embedding_values.value(i))
            .collect();

        let metadata_json = batch
            .column(5)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| AppError::Knowledge("Invalid metadata column".to_string()))?
            .value(row_idx);

        let metadata: serde_json::Value = serde_json::from_str(metadata_json)
            .map_err(|e| AppError::Knowledge(format!("Failed to parse metadata: {}", e)))?;

        Ok(KnowledgeChunk {
            id,
            source_id,
            position,
            text,
            embedding: Some(embedding),
            metadata,
        })
    }
}

impl VectorIndex for LanceDbIndex {
    fn upsert_chunk(&mut self, chunk: &KnowledgeChunk) -> AppResult<()> {
        // Track source ID
        self.source_ids.insert(chunk.source_id.clone());

        let batch = self.chunk_to_batch(chunk)?;

        // Use blocking runtime for sync trait
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.table
                    .add(RecordBatchIterator::new(
                        vec![Ok(batch.clone())],
                        batch.schema(),
                    ))
                    .execute()
                    .await
                    .map_err(|e| AppError::Knowledge(format!("Failed to add chunk: {}", e)))?;
                Ok::<(), AppError>(())
            })
        })?;

        Ok(())
    }

    fn search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> AppResult<Vec<(KnowledgeChunk, f32)>> {
        if query_embedding.len() != self.embedding_dim {
            return Err(AppError::Knowledge(format!(
                "Query embedding dimension mismatch: expected {}, got {}",
                self.embedding_dim,
                query_embedding.len()
            )));
        }

        let query_vec = query_embedding.to_vec();
        let batches = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                use futures::TryStreamExt;

                self.table
                    .query()
                    .nearest_to(query_vec.clone())
                    .map_err(|e| AppError::Knowledge(format!("Failed to create query: {}", e)))?
                    .limit(top_k)
                    .execute()
                    .await
                    .map_err(|e| AppError::Knowledge(format!("Failed to execute search: {}", e)))?
                    .try_collect::<Vec<_>>()
                    .await
                    .map_err(|e| AppError::Knowledge(format!("Failed to collect results: {}", e)))
            })
        })?;

        let mut chunks_with_scores = Vec::new();

        // Process batches
        for batch in batches {
            for row_idx in 0..batch.num_rows() {
                let chunk = self.batch_to_chunk(&batch, row_idx)?;

                // Calculate cosine similarity score
                let score = if let Some(embedding) = &chunk.embedding {
                    cosine_similarity(query_embedding, embedding)
                } else {
                    0.0
                };

                chunks_with_scores.push((chunk, score));
            }
        }

        // Sort by score descending
        chunks_with_scores
            .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        tracing::debug!(
            "Retrieved {} chunks (requested top-{})",
            chunks_with_scores.len(),
            top_k
        );

        Ok(chunks_with_scores)
    }

    fn stats(&self) -> AppResult<(u32, u32)> {
        let count = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.table
                    .count_rows(None)
                    .await
                    .map_err(|e| AppError::Knowledge(format!("Failed to count rows: {}", e)))
            })
        })?;

        let sources_count = self.source_ids.len() as u32;
        let chunks_count = count as u32;

        Ok((sources_count, chunks_count))
    }

    fn reset(&mut self) -> AppResult<()> {
        // Drop and recreate table
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                // LanceDB doesn't have a direct drop table method in the public API
                // We'll delete all rows instead
                let count = self.table.count_rows(None).await.unwrap_or(0);

                if count > 0 {
                    // Delete all rows by creating a predicate that matches everything
                    self.table.delete("id IS NOT NULL").await.map_err(|e| {
                        AppError::Knowledge(format!("Failed to reset index: {}", e))
                    })?;
                }

                Ok::<(), AppError>(())
            })
        })?;

        self.source_ids.clear();
        tracing::info!("Reset LanceDB index");

        Ok(())
    }

    fn flush(&mut self) -> AppResult<()> {
        // LanceDB handles persistence automatically
        Ok(())
    }
}

/// Calculate cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot_product / (norm_a * norm_b)
}
