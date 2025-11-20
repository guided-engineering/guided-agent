//! Ollama LLM provider implementation.
//!
//! This module provides integration with Ollama, a local LLM runtime.
//! Ollama API: https://github.com/ollama/ollama/blob/main/docs/api.md

use crate::client::{LlmClient, LlmRequest, LlmResponse, LlmStream, LlmStreamChunk, LlmUsage};
use futures::StreamExt;
use guided_core::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// Ollama API request format.
#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<u32>,
    stream: bool,
}

/// Ollama API response format.
#[derive(Debug, Deserialize)]
struct OllamaResponse {
    model: String,
    response: String,
    done: bool,
    #[serde(default)]
    prompt_eval_count: Option<u32>,
    #[serde(default)]
    eval_count: Option<u32>,
}

/// Ollama LLM client.
pub struct OllamaClient {
    /// Base URL for Ollama API
    base_url: String,

    /// HTTP client
    client: reqwest::Client,
}

impl OllamaClient {
    /// Create a new Ollama client with default settings.
    ///
    /// Default URL: http://localhost:11434
    pub fn new() -> Self {
        Self::with_base_url("http://localhost:11434")
    }

    /// Create a new Ollama client with a custom base URL.
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            client: reqwest::Client::new(),
        }
    }

    /// Convert LlmRequest to Ollama format.
    fn to_ollama_request(&self, request: &LlmRequest) -> OllamaRequest {
        OllamaRequest {
            model: request.model.clone(),
            prompt: request.prompt.clone(),
            system: request.system.clone(),
            temperature: request.temperature,
            num_predict: request.max_tokens,
            stream: request.stream,
        }
    }

    /// Convert Ollama response to LlmResponse.
    fn convert_response(&self, response: OllamaResponse) -> LlmResponse {
        let usage = LlmUsage::new(
            response.prompt_eval_count.unwrap_or(0),
            response.eval_count.unwrap_or(0),
        );

        LlmResponse {
            content: response.response,
            model: response.model,
            usage,
            done: response.done,
        }
    }
}

impl Default for OllamaClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl LlmClient for OllamaClient {
    fn provider_name(&self) -> &str {
        "ollama"
    }

    async fn complete(&self, request: &LlmRequest) -> AppResult<LlmResponse> {
        tracing::info!("Sending completion request to Ollama");
        tracing::debug!("Request: {:?}", request);

        let ollama_request = self.to_ollama_request(request);
        let url = format!("{}/api/generate", self.base_url);

        let response = self
            .client
            .post(&url)
            .json(&ollama_request)
            .send()
            .await
            .map_err(|e| AppError::Llm(format!("Failed to send request to Ollama: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AppError::Llm(format!(
                "Ollama API error ({}): {}",
                status, error_text
            )));
        }

        // For non-streaming, Ollama returns a single JSON object
        let ollama_response: OllamaResponse = response
            .json()
            .await
            .map_err(|e| AppError::Llm(format!("Failed to parse Ollama response: {}", e)))?;

        tracing::info!("Received completion from Ollama");
        tracing::debug!("Response: {:?}", ollama_response);

        Ok(self.convert_response(ollama_response))
    }

    async fn stream(&self, request: &LlmRequest) -> AppResult<LlmStream> {
        tracing::info!("Starting streaming request to Ollama");
        tracing::debug!("Request: {:?}", request);

        let mut ollama_request = self.to_ollama_request(request);
        ollama_request.stream = true; // Ensure streaming is enabled

        let url = format!("{}/api/generate", self.base_url);

        let response = self
            .client
            .post(&url)
            .json(&ollama_request)
            .send()
            .await
            .map_err(|e| AppError::Llm(format!("Failed to send streaming request: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AppError::Llm(format!(
                "Ollama API error ({}): {}",
                status, error_text
            )));
        }

        // Convert byte stream to line-delimited JSON chunks
        let stream = response.bytes_stream().map(move |result| {
            let bytes = result.map_err(|e| AppError::Llm(format!("Stream error: {}", e)))?;

            // Parse each line as JSON (Ollama sends newline-delimited JSON)
            let text = String::from_utf8_lossy(&bytes);
            let chunks: Vec<AppResult<LlmStreamChunk>> = text
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| {
                    let ollama_response: OllamaResponse = serde_json::from_str(line)
                        .map_err(|e| AppError::Llm(format!("Failed to parse chunk: {}", e)))?;

                    Ok(LlmStreamChunk {
                        content: ollama_response.response,
                        model: ollama_response.model,
                        done: ollama_response.done,
                        usage: if ollama_response.done {
                            Some(LlmUsage::new(
                                ollama_response.prompt_eval_count.unwrap_or(0),
                                ollama_response.eval_count.unwrap_or(0),
                            ))
                        } else {
                            None
                        },
                    })
                })
                .collect();

            // Return chunks as a stream
            Ok(futures::stream::iter(chunks))
        });

        Ok(Box::pin(stream.flat_map(|result| match result {
            Ok(chunks) => chunks,
            Err(e) => futures::stream::iter(vec![Err(e)]),
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_client_creation() {
        let client = OllamaClient::new();
        assert_eq!(client.provider_name(), "ollama");
        assert_eq!(client.base_url, "http://localhost:11434");
    }

    #[test]
    fn test_ollama_request_conversion() {
        let client = OllamaClient::new();
        let request = LlmRequest::new("Hello", "llama3")
            .with_temperature(0.7)
            .with_max_tokens(100);

        let ollama_req = client.to_ollama_request(&request);
        assert_eq!(ollama_req.model, "llama3");
        assert_eq!(ollama_req.prompt, "Hello");
        assert_eq!(ollama_req.temperature, Some(0.7));
        assert_eq!(ollama_req.num_predict, Some(100));
    }
}
