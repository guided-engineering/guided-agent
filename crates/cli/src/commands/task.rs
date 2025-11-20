//! Task command handler.
//!
//! Handles multi-step task planning and execution.

use clap::{Args, Subcommand};
use guided_core::AppResult;
use std::path::PathBuf;

/// Multi-step task planning and execution
#[derive(Args, Debug)]
pub struct TaskCommand {
    #[command(subcommand)]
    pub action: TaskAction,
}

#[derive(Subcommand, Debug)]
pub enum TaskAction {
    /// Create a new task plan
    Plan(TaskPlanCommand),
    /// Execute a task plan
    Run(TaskRunCommand),
    /// Show task details
    Show(TaskShowCommand),
}

/// Create a new task plan
#[derive(Args, Debug)]
pub struct TaskPlanCommand {
    /// Natural language description of the task
    pub description: Option<String>,

    /// Description text (explicit flag)
    #[arg(short, long, conflicts_with = "description")]
    pub prompt: Option<String>,

    /// Read task description from file
    #[arg(short, long)]
    pub file: Option<PathBuf>,

    /// Explicit task identifier
    #[arg(long)]
    pub id: Option<String>,

    /// Overwrite existing plan with same ID
    #[arg(long)]
    pub overwrite: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

impl TaskPlanCommand {
    pub async fn execute(&self) -> AppResult<()> {
        tracing::info!("Executing task plan command");
        tracing::debug!("Task plan options: {:?}", self);

        // TODO: Implement task planning in future phases
        // 1. Load task description
        // 2. Generate TaskPlan via LLM
        // 3. Save to .guided/tasks/<id>.json

        println!("Task plan command not yet implemented");
        println!("Description: {:?}", self.get_description());

        Ok(())
    }

    fn get_description(&self) -> Option<String> {
        self.description
            .clone()
            .or_else(|| self.prompt.clone())
            .or_else(|| {
                self.file.as_ref().and_then(|path| {
                    std::fs::read_to_string(path)
                        .map_err(|e| tracing::error!("Failed to read task file: {}", e))
                        .ok()
                })
            })
    }
}

/// Execute a task plan
#[derive(Args, Debug)]
pub struct TaskRunCommand {
    /// Task ID to execute
    #[arg(long)]
    pub id: String,

    /// Do not modify files, simulate actions
    #[arg(long)]
    pub dry_run: bool,

    /// Execute a specific step only
    #[arg(long)]
    pub step: Option<usize>,

    /// Execute up to a specific step
    #[arg(long)]
    pub until_step: Option<usize>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

impl TaskRunCommand {
    pub async fn execute(&self) -> AppResult<()> {
        tracing::info!("Executing task run command for task: {}", self.id);
        tracing::debug!("Task run options: {:?}", self);

        // TODO: Implement task execution in future phases
        // 1. Load TaskPlan from .guided/tasks/<id>.json
        // 2. Execute each TaskStep
        // 3. Log results to .guided/tasks/<id>.log.json

        println!("Task run command not yet implemented");
        println!("Task ID: {}", self.id);

        Ok(())
    }
}

/// Show task details
#[derive(Args, Debug)]
pub struct TaskShowCommand {
    /// Task ID to display
    #[arg(long)]
    pub id: String,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

impl TaskShowCommand {
    pub async fn execute(&self) -> AppResult<()> {
        tracing::info!("Executing task show command for task: {}", self.id);
        tracing::debug!("Task show options: {:?}", self);

        // TODO: Implement task display in future phases
        // 1. Load TaskPlan from .guided/tasks/<id>.json
        // 2. Load execution logs if available
        // 3. Display in requested format

        println!("Task show command not yet implemented");
        println!("Task ID: {}", self.id);

        Ok(())
    }
}

impl TaskCommand {
    pub async fn execute(&self) -> AppResult<()> {
        match &self.action {
            TaskAction::Plan(cmd) => cmd.execute().await,
            TaskAction::Run(cmd) => cmd.execute().await,
            TaskAction::Show(cmd) => cmd.execute().await,
        }
    }
}
