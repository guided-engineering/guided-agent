//! Guided Agent CLI
//!
//! Main entry point for the guided command-line tool.
//! Provides commands for AI-assisted development with local-first RAG.

mod commands;

use clap::{Parser, Subcommand};
use commands::{AskCommand, KnowledgeCommand, StatsCommand, TaskCommand};
use guided_core::{config::AppConfig, logging, AppResult};
use std::path::PathBuf;

/// Guided Agent CLI - AI-assisted development with local-first RAG
#[derive(Parser, Debug)]
#[command(name = "guided")]
#[command(about = "AI-assisted development with local-first RAG", long_about = None)]
#[command(version)]
struct Cli {
    /// Path to workspace directory (default: current directory)
    #[arg(short, long, global = true, env = "GUIDED_WORKSPACE")]
    workspace: Option<PathBuf>,

    /// Path to config file
    #[arg(short, long, global = true, env = "GUIDED_CONFIG")]
    config: Option<PathBuf>,

    /// Log level (error, warn, info, debug, trace)
    #[arg(long, global = true, env = "RUST_LOG")]
    log_level: Option<String>,

    /// Enable verbose output (sets log level to debug)
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Disable colored output
    #[arg(long, global = true, env = "NO_COLOR")]
    no_color: bool,

    /// LLM provider (openai, anthropic, ollama, etc.)
    #[arg(short, long, global = true, env = "GUIDED_PROVIDER")]
    provider: Option<String>,

    /// Model identifier
    #[arg(short, long, global = true, env = "GUIDED_MODEL")]
    model: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Ask a question with optional context
    Ask(AskCommand),

    /// Multi-step task planning and execution
    Task(TaskCommand),

    /// Knowledge base management (local RAG)
    Knowledge(KnowledgeCommand),

    /// Show usage statistics
    Stats(StatsCommand),
}

#[tokio::main]
async fn main() -> AppResult<()> {
    // Parse command-line arguments first (needed for logging config)
    let cli = Cli::parse();

    // Load base configuration from environment
    let config = AppConfig::load()?;

    // Apply CLI overrides
    let config = config.with_overrides(
        cli.workspace,
        cli.config,
        cli.provider,
        cli.model,
        cli.log_level,
        cli.verbose,
        cli.no_color,
    );

    // Initialize logging with final configuration
    logging::init_logging(config.log_level.as_deref(), config.no_color)?;

    // Log startup
    tracing::info!("Guided Agent CLI starting");
    tracing::debug!("Workspace: {:?}", config.workspace);
    tracing::debug!("Provider: {}", config.provider);
    tracing::debug!("Model: {}", config.model);

    // Ensure .guided directory exists
    config.ensure_guided_dir()?;

    // Emit command.start span
    let command_name = match &cli.command {
        Commands::Ask(_) => "ask",
        Commands::Task(_) => "task",
        Commands::Knowledge(_) => "knowledge",
        Commands::Stats(_) => "stats",
    };
    let _span = tracing::info_span!("command", name = command_name).entered();

    // Route to command handlers
    let result = match cli.command {
        Commands::Ask(cmd) => cmd.execute(&config).await,
        Commands::Task(cmd) => cmd.execute().await,
        Commands::Knowledge(cmd) => cmd.execute().await,
        Commands::Stats(cmd) => cmd.execute().await,
    };

    // Log completion
    match &result {
        Ok(_) => tracing::info!("Command completed successfully"),
        Err(e) => tracing::error!("Command failed: {}", e),
    }

    result
}
