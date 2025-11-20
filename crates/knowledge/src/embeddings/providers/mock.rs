//! Mock embedding provider using trigram-based content-aware embeddings.

use crate::embeddings::provider::EmbeddingProvider;
use guided_core::AppResult;

/// Mock provider for testing and development.
///
/// Generates deterministic embeddings based on text content using
/// character trigrams and word frequencies. While not semantically
/// accurate like real embedding models, it produces consistent,
/// content-dependent vectors suitable for testing.
#[derive(Debug)]
pub struct MockProvider {
    dimensions: usize,
}

impl MockProvider {
    /// Create a new mock provider with specified dimensions.
    pub fn new(dimensions: usize) -> Self {
        Self { dimensions }
    }

    /// Generate a mock embedding for text using trigram-based approach.
    fn generate_mock_embedding(&self, text: &str) -> AppResult<Vec<f32>> {
        let mut embedding = vec![0.0; self.dimensions];

        // Use text properties to create content-aware embeddings
        let lower = text.to_lowercase();

        // Filter stop words for better discrimination
        let stop_words: std::collections::HashSet<&str> = [
            "the", "is", "at", "which", "on", "a", "an", "as", "are", "was", "were", "for", "to",
            "of", "in", "and", "or", "but", "with", "by", "from", "this", "that", "be", "have",
            "has", "had", "it", "its", "their", "they", "them",
        ]
        .iter()
        .copied()
        .collect();

        let words: Vec<&str> = lower
            .split_whitespace()
            .filter(|w| !stop_words.contains(w) && w.len() > 2)
            .collect();

        // Build word frequency map
        let mut word_freq = std::collections::HashMap::new();
        for word in &words {
            *word_freq.entry(*word).or_insert(0) += 1;
        }

        // Map each unique word to multiple dimensions based on character trigrams
        // This creates more specific semantic vectors
        for (word, freq) in word_freq.iter() {
            // Use character trigrams for better semantic encoding
            let chars: Vec<char> = word.chars().collect();
            for i in 0..chars.len().saturating_sub(2) {
                let trigram = format!(
                    "{}{}{}",
                    chars[i],
                    chars[i + 1],
                    chars.get(i + 2).unwrap_or(&' ')
                );
                let trigram_hash = trigram
                    .bytes()
                    .fold(0u64, |acc, b| acc.wrapping_mul(37).wrapping_add(b as u64));

                let dim_idx = (trigram_hash as usize) % self.dimensions;
                embedding[dim_idx] += (*freq as f32).sqrt(); // sqrt scale for better distribution
            }

            // Also encode whole word
            let word_hash = word
                .bytes()
                .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
            let base_dim = (word_hash as usize) % self.dimensions;
            embedding[base_dim] += *freq as f32;
        }

        // Normalize to unit vector
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut embedding {
                *v /= norm;
            }
        }

        Ok(embedding)
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
        texts
            .iter()
            .map(|text| self.generate_mock_embedding(text))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_provider_dimensions() {
        let provider = MockProvider::new(384);
        assert_eq!(provider.dimensions(), 384);
        assert_eq!(provider.provider_name(), "mock");
        assert_eq!(provider.model_name(), "trigram-v1");
    }

    #[tokio::test]
    async fn test_mock_provider_embed_single() {
        let provider = MockProvider::new(384);
        let embedding = provider.embed("hello world").await.unwrap();

        assert_eq!(embedding.len(), 384);

        // Verify normalization (unit vector)
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_mock_provider_embed_batch() {
        let provider = MockProvider::new(384);
        let texts = vec![
            "hello world".to_string(),
            "test embedding".to_string(),
            "rust programming".to_string(),
        ];

        let embeddings = provider.embed_batch(&texts).await.unwrap();

        assert_eq!(embeddings.len(), 3);
        for embedding in &embeddings {
            assert_eq!(embedding.len(), 384);

            // Verify normalization
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!((norm - 1.0).abs() < 0.001);
        }
    }

    #[tokio::test]
    async fn test_mock_provider_deterministic() {
        let provider = MockProvider::new(384);
        let text = "deterministic test";

        let embedding1 = provider.embed(text).await.unwrap();
        let embedding2 = provider.embed(text).await.unwrap();

        // Same text should produce identical embeddings
        assert_eq!(embedding1, embedding2);
    }

    #[tokio::test]
    async fn test_mock_provider_different_texts() {
        let provider = MockProvider::new(384);

        let embedding1 = provider.embed("hello world").await.unwrap();
        let embedding2 = provider.embed("goodbye world").await.unwrap();

        // Different texts should produce different embeddings
        assert_ne!(embedding1, embedding2);
    }

    #[tokio::test]
    async fn test_mock_provider_empty_text() {
        let provider = MockProvider::new(384);
        let embedding = provider.embed("").await.unwrap();

        assert_eq!(embedding.len(), 384);
        // Empty text should produce zero vector
        assert!(embedding.iter().all(|&x| x == 0.0));
    }

    #[tokio::test]
    async fn test_mock_provider_utf8_safety() {
        let provider = MockProvider::new(384);

        // Test with Brazilian Portuguese and emojis
        let text = "Gamedex é um aplicativo 🎮 brasileiro para gerenciar jogos!";
        let embedding = provider.embed(text).await.unwrap();

        assert_eq!(embedding.len(), 384);

        // Verify normalization
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.001);
    }
}
