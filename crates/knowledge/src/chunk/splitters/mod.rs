//! Splitter implementations wrapper module.

mod code;
mod fallback;
mod text;

pub use code::CodeSplitter;
pub use fallback::FallbackSplitter;
pub use text::TextSplitter;

use crate::chunk::{Chunk, ChunkConfig};
use guided_core::AppResult;

/// Trait for chunk splitters.
pub trait ChunkSplitter {
    /// Split text into semantic chunks.
    fn split(&self, source_id: &str, text: &str, config: &ChunkConfig) -> AppResult<Vec<Chunk>>;
}
