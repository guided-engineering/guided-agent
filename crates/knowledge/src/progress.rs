//! Structured progress reporting for knowledge operations.
//!
//! Provides observable, incremental feedback during long-running operations like
//! indexing, embedding, and chunking.

use std::sync::Arc;
use std::time::Instant;

/// Progress event emitted during knowledge operations.
#[derive(Debug, Clone)]
pub struct ProgressEvent {
    /// Phase of the operation: "discover", "parse", "chunk", "embed", "index"
    pub phase: String,
    
    /// Current progress (files processed, chunks created, etc.)
    pub current: u64,
    
    /// Total expected work (if known)
    pub total: Option<u64>,
    
    /// Percentage complete (0.0 - 100.0)
    pub percentage: Option<f64>,
    
    /// Human-readable message
    pub message: String,
    
    /// Elapsed time since phase started
    pub elapsed_secs: Option<f64>,
}

impl ProgressEvent {
    /// Create a new progress event.
    pub fn new(phase: impl Into<String>, current: u64, total: Option<u64>, message: impl Into<String>) -> Self {
        let percentage = total.map(|t| if t > 0 { (current as f64 / t as f64) * 100.0 } else { 0.0 });
        
        Self {
            phase: phase.into(),
            current,
            total,
            percentage,
            message: message.into(),
            elapsed_secs: None,
        }
    }
    
    /// Set elapsed time.
    pub fn with_elapsed(mut self, elapsed_secs: f64) -> Self {
        self.elapsed_secs = Some(elapsed_secs);
        self
    }
    
    /// Format as a simple user-facing line.
    pub fn format_simple(&self) -> String {
        let progress = if let Some(total) = self.total {
            format!("{}/{}", self.current, total)
        } else {
            format!("{}", self.current)
        };
        
        let pct = if let Some(p) = self.percentage {
            format!(" ({:.0}%)", p)
        } else {
            String::new()
        };
        
        format!("[{}] {}{} - {}", self.phase, progress, pct, self.message)
    }
}

/// Callback for progress events.
pub type ProgressCallback = Arc<dyn Fn(ProgressEvent) + Send + Sync>;

/// Progress reporter that emits events through a callback.
#[derive(Clone)]
pub struct ProgressReporter {
    callback: Option<ProgressCallback>,
    start_time: Arc<Instant>,
}

impl ProgressReporter {
    /// Create a new reporter with a callback.
    pub fn new(callback: ProgressCallback) -> Self {
        Self {
            callback: Some(callback),
            start_time: Arc::new(Instant::now()),
        }
    }
    
    /// Create a no-op reporter (no events emitted).
    pub fn noop() -> Self {
        Self {
            callback: None,
            start_time: Arc::new(Instant::now()),
        }
    }
    
    /// Emit a progress event.
    pub fn emit(&self, event: ProgressEvent) {
        if let Some(callback) = &self.callback {
            // Add elapsed time
            let elapsed = self.start_time.elapsed().as_secs_f64();
            let event_with_time = event.with_elapsed(elapsed);
            
            // Log to tracing
            tracing::debug!(
                phase = %event_with_time.phase,
                current = event_with_time.current,
                total = ?event_with_time.total,
                percentage = ?event_with_time.percentage,
                message = %event_with_time.message,
                elapsed_secs = elapsed,
                "Progress event"
            );
            
            // Call callback
            callback(event_with_time);
        }
    }
    
    /// Emit discovery phase event.
    pub fn discover(&self, current: u64, total: Option<u64>, path: &str) {
        self.emit(ProgressEvent::new(
            "discover",
            current,
            total,
            format!("scanning {}", path),
        ));
    }
    
    /// Emit parsing phase event.
    pub fn parse(&self, current: u64, total: Option<u64>, file: &str) {
        self.emit(ProgressEvent::new(
            "parse",
            current,
            total,
            format!("reading {}", file),
        ));
    }
    
    /// Emit chunking phase event.
    pub fn chunk(&self, current: u64, total: Option<u64>, chunks_created: u32) {
        self.emit(ProgressEvent::new(
            "chunk",
            current,
            total,
            format!("{} chunks created", chunks_created),
        ));
    }
    
    /// Emit embedding phase event.
    pub fn embed(&self, current: u64, total: Option<u64>, model: &str) {
        self.emit(ProgressEvent::new(
            "embed",
            current,
            total,
            format!("model={}", model),
        ));
    }
    
    /// Emit indexing phase event.
    pub fn index(&self, current: u64, total: Option<u64>) {
        self.emit(ProgressEvent::new(
            "index",
            current,
            total,
            "writing to LanceDB",
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_progress_event_format() {
        let event = ProgressEvent::new("discover", 5, Some(10), "scanning files");
        let formatted = event.format_simple();
        assert!(formatted.contains("[discover]"));
        assert!(formatted.contains("5/10"));
        assert!(formatted.contains("50%"));
    }

    #[test]
    fn test_progress_reporter_emit() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();
        
        let reporter = ProgressReporter::new(Arc::new(move |event| {
            events_clone.lock().unwrap().push(event);
        }));
        
        reporter.discover(3, Some(10), "/path/to/file");
        
        let captured = events.lock().unwrap();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].phase, "discover");
        assert_eq!(captured[0].current, 3);
    }

    #[test]
    fn test_noop_reporter() {
        let reporter = ProgressReporter::noop();
        reporter.discover(1, None, "test"); // Should not panic
    }
}
