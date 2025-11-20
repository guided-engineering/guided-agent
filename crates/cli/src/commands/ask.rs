//! Ask command handler.
//!
//! Handles LLM queries with optional workspace and knowledge context.

use clap::Args;
use futures::StreamExt;
use guided_core::{config::AppConfig, AppResult};
use guided_llm::{create_client, LlmClient, LlmRequest};
use guided_prompt::{build_prompt, load_prompt};
use std::collections::HashMap;
use std::path::PathBuf;

/// Ask a question with optional context
#[derive(Args, Debug)]
pub struct AskCommand {
    /// The question to ask (alternative to --prompt flag)
    pub prompt: Option<String>,

    /// Question text (explicit flag)
    #[arg(long, conflicts_with = "prompt")]
    pub prompt_flag: Option<String>,

    /// Read prompt from file
    #[arg(short, long)]
    pub file: Option<PathBuf>,

    /// Knowledge base to query for context
    #[arg(short, long)]
    pub knowledge_base: Option<String>,

    /// Include workspace context (file tree, metadata)
    #[arg(long)]
    pub with_workspace: bool,

    /// Enable streaming (default: true)
    #[arg(long, default_value = "true")]
    pub stream: bool,

    /// Disable streaming
    #[arg(long, conflicts_with = "stream")]
    pub no_stream: bool,

    /// Maximum tokens in response
    #[arg(long)]
    pub max_tokens: Option<u32>,

    /// Temperature for response generation (0.0-2.0)
    #[arg(long)]
    pub temperature: Option<f32>,

    /// Output format (markdown, text, json)
    #[arg(short = 'o', long, default_value = "markdown")]
    pub format: String,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

impl AskCommand {
    /// Execute the ask command.
    pub async fn execute(&self, config: &AppConfig) -> AppResult<()> {
        tracing::info!("Executing ask command");
        tracing::debug!("Ask command options: {:?}", self);

        // 1. Get the user input
        let user_input = self
            .get_prompt()
            .ok_or_else(|| guided_core::AppError::Config("No prompt provided".to_string()))?;

        tracing::debug!("User input: {}", user_input);

        // 2. Load prompt definition
        let mut prompt_def = load_prompt(&config.workspace, "agent.ask.default")?;
        tracing::debug!("Loaded prompt definition: {}", prompt_def.id);

        // 3. Override context settings based on CLI flags
        if self.with_workspace {
            prompt_def.context.include_workspace_context = true;
            tracing::debug!("Workspace context enabled via --with-workspace flag");
        }

        if self.knowledge_base.is_some() {
            prompt_def.context.include_knowledge_base = true;
        }

        // 4. Build prompt with variables
        let mut variables = HashMap::new();
        variables.insert("prompt".to_string(), user_input);

        // 5. Fetch knowledge base context if requested
        let knowledge_context = if let Some(ref kb_name) = self.knowledge_base {
            tracing::info!("Retrieving knowledge from base: {}", kb_name);

            match self.retrieve_knowledge(config, kb_name).await {
                Ok(context) => {
                    tracing::debug!("Retrieved {} bytes of knowledge context", context.len());
                    Some(context)
                }
                Err(e) => {
                    tracing::warn!("Knowledge base retrieval failed: {}", e);
                    return Err(guided_core::AppError::Knowledge(format!(
                        "Failed to retrieve from knowledge base '{}': {}. Use 'guided knowledge learn' to populate the base.",
                        kb_name, e
                    )));
                }
            }
        } else {
            None
        };

        let built_prompt =
            build_prompt(&prompt_def, variables, &config.workspace, knowledge_context)?;

        tracing::debug!(
            "Built prompt - workspace context: {}, knowledge: {:?}",
            built_prompt.metadata.workspace_context_included,
            built_prompt.metadata.knowledge_base_used
        );

        // 4. Get provider configuration
        let provider_config = config.get_provider_config(&config.provider)?;

        // 5. Resolve endpoint
        let endpoint = if let Some(ref pc) = provider_config {
            match pc {
                guided_core::config::ProviderConfig::Ollama { endpoint, .. } => {
                    Some(endpoint.as_str())
                }
                guided_core::config::ProviderConfig::OpenAI { endpoint, .. } => endpoint.as_deref(),
                guided_core::config::ProviderConfig::Claude { endpoint, .. } => endpoint.as_deref(),
                _ => None,
            }
        } else {
            None
        };

        // 6. Resolve API key
        let api_key = config.resolve_api_key(&config.provider)?;

        // 7. Create LLM client via factory
        let client = create_client(&config.provider, endpoint, api_key.as_deref())
            .map_err(guided_core::AppError::Config)?;

        // 8. Build LLM request from built prompt
        let mut request = LlmRequest::new(built_prompt.user, &config.model);

        if let Some(system) = built_prompt.system {
            request = request.with_system(system);
        }

        if let Some(max_tokens) = self.max_tokens {
            request = request.with_max_tokens(max_tokens);
        }

        if let Some(temperature) = self.temperature {
            request = request.with_temperature(temperature);
        }

        // 9. Execute request (streaming or non-streaming)
        if self.is_streaming() {
            self.handle_streaming(client.as_ref(), &request, &built_prompt.metadata, config)
                .await
        } else {
            self.handle_non_streaming(client.as_ref(), &request, &built_prompt.metadata, config)
                .await
        }
    }

    /// Handle non-streaming response.
    async fn handle_non_streaming(
        &self,
        client: &dyn LlmClient,
        request: &LlmRequest,
        built_prompt_metadata: &guided_prompt::BuiltPromptMetadata,
        config: &AppConfig,
    ) -> AppResult<()> {
        tracing::info!("Sending non-streaming request to LLM");

        let response = client.complete(request).await?;

        if self.json {
            // Output as structured JSON with metadata
            let output = serde_json::json!({
                "answer": response.content,
                "model": response.model,
                "provider": config.provider,
                "usage": {
                    "promptTokens": response.usage.prompt_tokens,
                    "completionTokens": response.usage.completion_tokens,
                    "totalTokens": response.usage.total_tokens
                },
                "metadata": {
                    "promptId": built_prompt_metadata.source_prompt_id,
                    "workspaceContext": built_prompt_metadata.workspace_context_included,
                    "knowledgeBase": built_prompt_metadata.knowledge_base_used
                }
            });

            let json = serde_json::to_string_pretty(&output)
                .map_err(|e| guided_core::AppError::Serialization(e.to_string()))?;
            println!("{}", json);
        } else {
            // Output as plain text to stdout
            println!("{}", response.content);

            // Show usage stats if verbose (to stderr)
            if tracing::enabled!(tracing::Level::DEBUG) {
                tracing::debug!(
                    "Token usage - Prompt: {}, Completion: {}, Total: {}",
                    response.usage.prompt_tokens,
                    response.usage.completion_tokens,
                    response.usage.total_tokens
                );
            }
        }

        Ok(())
    }

    /// Handle streaming response.
    async fn handle_streaming(
        &self,
        client: &dyn LlmClient,
        request: &LlmRequest,
        built_prompt_metadata: &guided_prompt::BuiltPromptMetadata,
        config: &AppConfig,
    ) -> AppResult<()> {
        tracing::info!("Starting streaming request to LLM");

        let mut stream = client.stream(request).await?;
        let mut full_content = String::new();
        let mut final_usage = None;

        while let Some(result) = stream.next().await {
            let chunk = result?;

            if !chunk.content.is_empty() {
                full_content.push_str(&chunk.content);

                if !self.json {
                    // Stream to stdout in real-time
                    print!("{}", chunk.content);
                    use std::io::Write;
                    std::io::stdout().flush().ok();
                }
            }

            if chunk.done {
                final_usage = chunk.usage;
                break;
            }
        }

        if self.json {
            // Output complete response as structured JSON
            let output = serde_json::json!({
                "answer": full_content,
                "model": request.model,
                "provider": config.provider,
                "usage": {
                    "promptTokens": final_usage.as_ref().map(|u| u.prompt_tokens).unwrap_or(0),
                    "completionTokens": final_usage.as_ref().map(|u| u.completion_tokens).unwrap_or(0),
                    "totalTokens": final_usage.as_ref().map(|u| u.total_tokens).unwrap_or(0)
                },
                "metadata": {
                    "promptId": built_prompt_metadata.source_prompt_id,
                    "workspaceContext": built_prompt_metadata.workspace_context_included,
                    "knowledgeBase": built_prompt_metadata.knowledge_base_used
                }
            });

            let json = serde_json::to_string_pretty(&output)
                .map_err(|e| guided_core::AppError::Serialization(e.to_string()))?;
            println!("{}", json);
        } else {
            // Add newline after streaming output
            println!();

            // Show usage stats if verbose (to stderr)
            if let Some(usage) = final_usage {
                if tracing::enabled!(tracing::Level::DEBUG) {
                    tracing::debug!(
                        "Token usage - Prompt: {}, Completion: {}, Total: {}",
                        usage.prompt_tokens,
                        usage.completion_tokens,
                        usage.total_tokens
                    );
                }
            }
        }

        Ok(())
    }

    /// Get the prompt text from various sources.
    fn get_prompt(&self) -> Option<String> {
        self.prompt
            .clone()
            .or_else(|| self.prompt_flag.clone())
            .or_else(|| {
                self.file.as_ref().and_then(|path| {
                    std::fs::read_to_string(path)
                        .map_err(|e| tracing::error!("Failed to read prompt file: {}", e))
                        .ok()
                })
            })
    }

    /// Check if streaming is enabled.
    #[allow(dead_code)]
    pub fn is_streaming(&self) -> bool {
        !self.no_stream && self.stream
    }

    /// Retrieve knowledge base context.
    async fn retrieve_knowledge(&self, config: &AppConfig, kb_name: &str) -> AppResult<String> {
        tracing::info!("Retrieving knowledge from base: {}", kb_name);

        // Use knowledge ask API to retrieve relevant chunks
        let api_key = config.resolve_api_key(&config.provider).ok().flatten();

        let options = guided_knowledge::AskOptions {
            base_name: kb_name.to_string(),
            query: self
                .get_prompt()
                .ok_or_else(|| guided_core::AppError::Config("No prompt provided".to_string()))?,
            top_k: 5, // Default to top 5 chunks
        };

        let result =
            guided_knowledge::ask(&config.workspace, options, api_key.as_deref()).await?;

        // Format chunks into context string
        let context = result
            .chunks
            .iter()
            .enumerate()
            .map(|(i, chunk)| {
                format!(
                    "[Chunk {}]\n{}\n",
                    i + 1,
                    chunk.text.trim()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        tracing::debug!(
            "Retrieved {} chunks ({} bytes) from knowledge base '{}'",
            result.chunks.len(),
            context.len(),
            kb_name
        );

        Ok(context)
    }
}
