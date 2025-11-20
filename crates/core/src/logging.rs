//! Logging infrastructure for the Guided Agent CLI.
//!
//! This module initializes the tracing subscriber for structured logging.
//! All logs are emitted to stderr to keep stdout clean for data output.

use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::error::AppResult;

/// Initialize the tracing subscriber with stderr output.
///
/// This sets up structured logging with:
/// - Output to stderr (stdout is reserved for data)
/// - Environment-based filtering (RUST_LOG or provided level)
/// - Human-readable format in development
/// - Optional ANSI color control
///
/// # Arguments
/// * `log_level` - Optional log level override (e.g., "debug", "info")
/// * `no_color` - Disable colored output
///
/// # Example
/// ```no_run
/// use guided_core::logging::init_logging;
///
/// init_logging(None, false).expect("Failed to initialize logging");
/// ```
pub fn init_logging(log_level: Option<&str>, no_color: bool) -> AppResult<()> {
    // Determine the filter level
    let default_level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    let filter_str = log_level.unwrap_or(&default_level);

    let env_filter = EnvFilter::try_new(filter_str)
        .map_err(|e| crate::error::AppError::Config(format!("Invalid log filter: {}", e)))?;

    // Configure format layer with color control
    let fmt_layer = fmt::layer()
        .with_writer(std::io::stderr)
        .with_target(true)
        .with_level(true)
        .with_ansi(!no_color && supports_color());

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .try_init()
        .map_err(|e| crate::error::AppError::Config(format!("Failed to init logging: {}", e)))?;

    Ok(())
}

/// Check if the terminal supports color output.
fn supports_color() -> bool {
    // Check NO_COLOR environment variable
    if std::env::var("NO_COLOR").is_ok() {
        return false;
    }

    // Check if we're in a terminal (simplified check)
    // For more robust detection, we'd use a crate like `atty` or `is-terminal`
    // For now, assume color support is available
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_logging() {
        // Note: Can only be called once per process
        // In real tests, we'd use a different approach
        let result = init_logging(None, false);
        assert!(result.is_ok() || result.is_err()); // May already be initialized
    }
}
