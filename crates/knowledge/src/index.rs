//! SQLite-backed vector index for knowledge chunks.

use crate::types::{KnowledgeChunk, KnowledgeSource};
use guided_core::{AppError, AppResult};
use rusqlite::{params, Connection};
use std::path::Path;

/// Initialize the SQLite index database.
pub fn init_index(db_path: &Path) -> AppResult<Connection> {
    // Ensure parent directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Knowledge(format!("Failed to create index directory: {}", e)))?;
    }

    let conn = Connection::open(db_path)
        .map_err(|e| AppError::Knowledge(format!("Failed to open SQLite index: {}", e)))?;

    // Create tables
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS sources (
            id TEXT PRIMARY KEY,
            path TEXT,
            url TEXT,
            content_type TEXT NOT NULL,
            learned_at TEXT NOT NULL,
            size_bytes INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS chunks (
            id TEXT PRIMARY KEY,
            source_id TEXT NOT NULL,
            position INTEGER NOT NULL,
            text TEXT NOT NULL,
            embedding BLOB NOT NULL,
            metadata TEXT,
            FOREIGN KEY (source_id) REFERENCES sources(id)
        );

        CREATE INDEX IF NOT EXISTS idx_chunks_source ON chunks(source_id);
        "#,
    )
    .map_err(|e| AppError::Knowledge(format!("Failed to create tables: {}", e)))?;

    tracing::debug!("Initialized SQLite index at {:?}", db_path);
    Ok(conn)
}

/// Insert a source into the index.
pub fn insert_source(conn: &Connection, source: &KnowledgeSource) -> AppResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO sources (id, path, url, content_type, learned_at, size_bytes) 
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            source.id,
            source
                .path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string()),
            source.url,
            source.content_type,
            source.learned_at.to_rfc3339(),
            source.size_bytes as i64,
        ],
    )
    .map_err(|e| AppError::Knowledge(format!("Failed to insert source: {}", e)))?;

    Ok(())
}

/// Insert a chunk with embedding into the index.
pub fn insert_chunk(conn: &Connection, chunk: &KnowledgeChunk) -> AppResult<()> {
    let embedding_bytes = embedding_to_bytes(
        chunk
            .embedding
            .as_ref()
            .ok_or_else(|| AppError::Knowledge("Chunk missing embedding".to_string()))?,
    )?;

    let metadata_json = serde_json::to_string(&chunk.metadata)
        .map_err(|e| AppError::Knowledge(format!("Failed to serialize metadata: {}", e)))?;

    conn.execute(
        "INSERT OR REPLACE INTO chunks (id, source_id, position, text, embedding, metadata) 
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            chunk.id,
            chunk.source_id,
            chunk.position as i64,
            chunk.text,
            embedding_bytes,
            metadata_json,
        ],
    )
    .map_err(|e| AppError::Knowledge(format!("Failed to insert chunk: {}", e)))?;

    Ok(())
}

/// Query the index for top-k most similar chunks.
pub fn query_chunks(
    conn: &Connection,
    query_embedding: &[f32],
    top_k: usize,
) -> AppResult<Vec<(KnowledgeChunk, f32)>> {
    // Note: query_embedding is used directly in cosine_similarity calculation below

    let mut stmt = conn
        .prepare("SELECT id, source_id, position, text, embedding, metadata FROM chunks")
        .map_err(|e| AppError::Knowledge(format!("Failed to prepare query: {}", e)))?;

    let chunks_iter = stmt
        .query_map([], |row| {
            let embedding_bytes: Vec<u8> = row.get(4)?;
            let embedding = bytes_to_embedding(&embedding_bytes)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

            let metadata_json: String = row.get(5)?;
            let metadata: serde_json::Value = serde_json::from_str(&metadata_json)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

            Ok(KnowledgeChunk {
                id: row.get(0)?,
                source_id: row.get(1)?,
                position: row.get::<_, i64>(2)? as u32,
                text: row.get(3)?,
                embedding: Some(embedding),
                metadata,
            })
        })
        .map_err(|e| AppError::Knowledge(format!("Failed to query chunks: {}", e)))?;

    let mut results: Vec<(KnowledgeChunk, f32)> = chunks_iter
        .filter_map(|r| r.ok())
        .map(|chunk| {
            let score = cosine_similarity(query_embedding, chunk.embedding.as_ref().unwrap());
            (chunk, score)
        })
        .collect();

    // Sort by score descending
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Take top-k
    results.truncate(top_k);

    tracing::debug!(
        "Retrieved {} chunks (requested top-{})",
        results.len(),
        top_k
    );

    Ok(results)
}

/// Get statistics for the index.
pub fn get_stats(conn: &Connection) -> AppResult<(u32, u32)> {
    let sources_count: u32 = conn
        .query_row("SELECT COUNT(*) FROM sources", [], |row| {
            row.get::<_, i64>(0).map(|v| v as u32)
        })
        .map_err(|e| AppError::Knowledge(format!("Failed to count sources: {}", e)))?;

    let chunks_count: u32 = conn
        .query_row("SELECT COUNT(*) FROM chunks", [], |row| {
            row.get::<_, i64>(0).map(|v| v as u32)
        })
        .map_err(|e| AppError::Knowledge(format!("Failed to count chunks: {}", e)))?;

    Ok((sources_count, chunks_count))
}

/// Reset the index (delete all data).
pub fn reset_index(conn: &Connection) -> AppResult<()> {
    conn.execute("DELETE FROM chunks", [])
        .map_err(|e| AppError::Knowledge(format!("Failed to delete chunks: {}", e)))?;

    conn.execute("DELETE FROM sources", [])
        .map_err(|e| AppError::Knowledge(format!("Failed to delete sources: {}", e)))?;

    tracing::info!("Reset knowledge base index");
    Ok(())
}

/// Convert embedding vector to bytes for storage.
fn embedding_to_bytes(embedding: &[f32]) -> AppResult<Vec<u8>> {
    let mut bytes = Vec::with_capacity(embedding.len() * 4);
    for &value in embedding {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    Ok(bytes)
}

/// Convert bytes back to embedding vector.
fn bytes_to_embedding(bytes: &[u8]) -> AppResult<Vec<f32>> {
    if bytes.len() % 4 != 0 {
        return Err(AppError::Knowledge(
            "Invalid embedding bytes length".to_string(),
        ));
    }

    let mut embedding = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        let value = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        embedding.push(value);
    }

    Ok(embedding)
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tempfile::NamedTempFile;

    #[test]
    fn test_init_index() {
        let temp_file = NamedTempFile::new().unwrap();
        let conn = init_index(temp_file.path()).unwrap();

        // Verify tables exist
        let table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert!(table_count >= 2); // sources and chunks tables
    }

    #[test]
    fn test_insert_and_query() {
        let temp_file = NamedTempFile::new().unwrap();
        let conn = init_index(temp_file.path()).unwrap();

        // Insert source
        let source = KnowledgeSource {
            id: "source1".to_string(),
            path: None,
            url: None,
            content_type: "text".to_string(),
            learned_at: Utc::now(),
            size_bytes: 100,
        };
        insert_source(&conn, &source).unwrap();

        // Insert chunk
        let chunk = KnowledgeChunk {
            id: "chunk1".to_string(),
            source_id: "source1".to_string(),
            position: 0,
            text: "test text".to_string(),
            embedding: Some(vec![1.0, 0.0, 0.0]),
            metadata: serde_json::json!({}),
        };
        insert_chunk(&conn, &chunk).unwrap();

        // Query
        let results = query_chunks(&conn, &[1.0, 0.0, 0.0], 5).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.id, "chunk1");
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![1.0, 0.0, 0.0];
        let d = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&c, &d) - 0.0).abs() < 0.001);
    }
}
