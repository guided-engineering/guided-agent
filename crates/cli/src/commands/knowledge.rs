//! Knowledge command handler.
//!
//! Handles local RAG knowledge base management.

use clap::{Args, Subcommand};
use guided_core::{config::AppConfig, AppResult};
use guided_knowledge::{AskOptions, LearnOptions};
use std::path::PathBuf;

/// Knowledge base management (local RAG)
#[derive(Args, Debug)]
pub struct KnowledgeCommand {
    #[command(subcommand)]
    pub action: KnowledgeAction,
}

#[derive(Subcommand, Debug)]
pub enum KnowledgeAction {
    /// Learn from sources (files, URLs, etc.)
    Learn(KnowledgeLearnCommand),
    /// Query the knowledge base
    Ask(KnowledgeAskCommand),
    /// Clean up knowledge base
    Clean(KnowledgeCleanCommand),
    /// Show knowledge base statistics
    Stats(KnowledgeStatsCommand),
}

/// Learn from sources
#[derive(Args, Debug)]
pub struct KnowledgeLearnCommand {
    /// Knowledge base name
    pub base: String,

    /// Paths to learn from
    #[arg(long)]
    pub path: Vec<PathBuf>,

    /// URLs to fetch and learn
    #[arg(long)]
    pub url: Vec<String>,

    /// Include patterns (glob)
    #[arg(long)]
    pub include: Vec<String>,

    /// Exclude patterns (glob)
    #[arg(long)]
    pub exclude: Vec<String>,

    /// Reset base before learning
    #[arg(long)]
    pub reset: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

impl KnowledgeLearnCommand {
    pub async fn execute(&self, config: &AppConfig) -> AppResult<()> {
        tracing::info!("Executing knowledge learn command for base '{}'", self.base);

        let options = LearnOptions {
            base_name: self.base.clone(),
            paths: self.path.clone(),
            urls: self.url.clone(),
            include: self.include.clone(),
            exclude: self.exclude.clone(),
            reset: self.reset,
        };

        let api_key = config.resolve_api_key(&config.provider).ok().flatten();

        let stats =
            guided_knowledge::learn(&config.workspace, options, api_key.as_deref()).await?;

        if self.json {
            let output = serde_json::json!({
                "base": self.base,
                "sourcesCount": stats.sources_count,
                "chunksCount": stats.chunks_count,
                "bytesProcessed": stats.bytes_processed,
                "durationSecs": stats.duration_secs,
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        } else {
            println!(
                "Learned {} sources ({} chunks, {} bytes) in {:.2}s",
                stats.sources_count, stats.chunks_count, stats.bytes_processed, stats.duration_secs
            );
        }

        Ok(())
    }
}

/// Query knowledge base
#[derive(Args, Debug)]
pub struct KnowledgeAskCommand {
    /// Knowledge base name
    pub base: String,

    /// Query text
    pub query: String,

    /// Number of chunks to retrieve
    #[arg(short = 'k', long, default_value = "5")]
    pub top_k: u32,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

impl KnowledgeAskCommand {
    pub async fn execute(&self, config: &AppConfig) -> AppResult<()> {
        tracing::info!(
            "Executing knowledge ask command for base '{}'",
            self.base
        );

        let options = AskOptions {
            base_name: self.base.clone(),
            query: self.query.clone(),
            top_k: self.top_k,
        };

        let api_key = config.resolve_api_key(&config.provider).ok().flatten();

        let result = guided_knowledge::ask(&config.workspace, options, api_key.as_deref()).await?;

        if self.json {
            let output = serde_json::json!({
                "base": self.base,
                "chunksCount": result.chunks.len(),
                "chunks": result.chunks.iter().map(|c| serde_json::json!({
                    "text": c.text,
                    "sourceId": c.source_id,
                    "position": c.position,
                })).collect::<Vec<_>>(),
                "scores": result.scores,
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        } else {
            println!("Retrieved {} chunks:", result.chunks.len());
            for (i, chunk) in result.chunks.iter().enumerate() {
                println!(
                    "\n[{}] Score: {:.3}\n{}",
                    i + 1,
                    result.scores.get(i).unwrap_or(&0.0),
                    chunk.text
                );
            }
        }

        Ok(())
    }
}

/// Clean knowledge base
#[derive(Args, Debug)]
pub struct KnowledgeCleanCommand {
    /// Knowledge base name
    pub base: String,
}

impl KnowledgeCleanCommand {
    pub async fn execute(&self, config: &AppConfig) -> AppResult<()> {
        tracing::info!("Executing knowledge clean command for base '{}'", self.base);

        guided_knowledge::clean(&config.workspace, &self.base)?;

        println!("Knowledge base '{}' cleaned", self.base);

        Ok(())
    }
}

/// Show knowledge base stats
#[derive(Args, Debug)]
pub struct KnowledgeStatsCommand {
    /// Knowledge base name
    pub base: String,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

impl KnowledgeStatsCommand {
    pub async fn execute(&self, config: &AppConfig) -> AppResult<()> {
        tracing::info!("Executing knowledge stats command for base '{}'", self.base);

        let stats = guided_knowledge::stats(&config.workspace, &self.base)?;

        if self.json {
            let output = serde_json::json!({
                "base": stats.base_name,
                "sourcesCount": stats.sources_count,
                "chunksCount": stats.chunks_count,
                "dbSizeBytes": stats.db_size_bytes,
                "lastLearnAt": stats.last_learn_at,
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        } else {
            println!("Knowledge base: {}", stats.base_name);
            println!("  Sources: {}", stats.sources_count);
            println!("  Chunks: {}", stats.chunks_count);
            println!("  DB size: {} bytes", stats.db_size_bytes);
            if let Some(last_learn) = stats.last_learn_at {
                println!("  Last learn: {}", last_learn);
            }
        }

        Ok(())
    }
}

impl KnowledgeCommand {
    pub async fn execute(&self, config: &AppConfig) -> AppResult<()> {
        match &self.action {
            KnowledgeAction::Learn(cmd) => cmd.execute(config).await,
            KnowledgeAction::Ask(cmd) => cmd.execute(config).await,
            KnowledgeAction::Clean(cmd) => cmd.execute(config).await,
            KnowledgeAction::Stats(cmd) => cmd.execute(config).await,
        }
    }
}
