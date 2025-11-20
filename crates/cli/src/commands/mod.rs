//! Command handlers for the Guided Agent CLI.
//!
//! This module organizes all CLI commands into separate submodules.

pub mod ask;
pub mod knowledge;
pub mod stats;
pub mod task;

// Re-export command types for convenience
pub use ask::AskCommand;
pub use knowledge::KnowledgeCommand;
pub use stats::StatsCommand;
pub use task::TaskCommand;
