//! OpenAI-compatible LLM provider implementation.
//!
//! This provider works with any API that follows the OpenAI Chat Completions format,
//! including:
//! - OpenAI (GPT-4, etc.)
//! - Qwen (通义千问) via DashScope
//! - DeepSeek
//! - Moonshot (Kimi)
//! - Local models via Ollama, vLLM, etc.
//!
//! Key concepts:
//! - **OpenAI Chat Completions API**: the de facto standard format that most
//!   LLM providers have adopted for compatibility
//!   POST {api_base}/chat/completions
//! - **api_base**: the base URL can be swapped to point at any compatible endpoint
//! - **Tool calling**: uses OpenAI's "tools" format with "function" type

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::LlmProvider;
use crate::types::{ChatRequest, ChatResponse, Role, ToolCall};

/// OpenAI-compatible API client.
///
/// Works with any provider that implements the OpenAI Chat Completions API format.
pub struct OpenAiCompatibleProvider {
    api_key: String,
    api_base: String,
    client: reqwest::Client,
}

// --- API Request Types (OpenAI format) ---

#[derive(Serialize)]
struct ApiRequest {
    model: String,
    messages: Vec<ApiMessage>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ApiTool>,
}

#[derive(Serialize)]
struct ApiMessage {
    role: String,
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ApiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Serialize)]
struct ApiTool {
    r#type: String,
    function: ApiFunction,
}

#[derive(Serialize)]
struct ApiFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ApiToolCall {
    id: String,
    r#type: String,
    function: ApiToolCallFunction,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ApiToolCallFunction {
    name: String,
    arguments: String,
}

// --- API Response Types ---

#[derive(Deserialize, Debug)]
struct ApiResponse {
    choices: Vec<ApiChoice>,
}

#[derive(Deserialize, Debug)]
struct ApiChoice {
    message: ApiResponseMessage,
}

#[derive(Deserialize, Debug)]
struct ApiResponseMessage {
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ApiToolCall>>,
}

// --- Implementation ---

impl OpenAiCompatibleProvider {
    pub fn new(api_key: String, api_base: Option<String>) -> Self {
        Self {
            api_key,
            api_base: api_base.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            client: reqwest::Client::new(),
        }
    }

    /// Convert internal messages to OpenAI API format.
    fn build_api_request(&self, request: &ChatRequest) -> ApiRequest {
        let mut api_messages: Vec<ApiMessage> = Vec::new();

        for msg in &request.messages {
            match msg.role {
                Role::System => {
                    api_messages.push(ApiMessage {
                        role: "system".to_string(),
                        content: Some(msg.content.clone()),
                        tool_calls: None,
                        tool_call_id: None,
                    });
                }
                Role::User => {
                    api_messages.push(ApiMessage {
                        role: "user".to_string(),
                        content: Some(msg.content.clone()),
                        tool_calls: None,
                        tool_call_id: None,
                    });
                }
                Role::Assistant => {
                    let tool_calls = if msg.tool_calls.is_empty() {
                        None
                    } else {
                        Some(
                            msg.tool_calls
                                .iter()
                                .map(|tc| ApiToolCall {
                                    id: tc.id.clone(),
                                    r#type: "function".to_string(),
                                    function: ApiToolCallFunction {
                                        name: tc.name.clone(),
                                        arguments: tc.arguments.clone(),
                                    },
                                })
                                .collect(),
                        )
                    };
                    api_messages.push(ApiMessage {
                        role: "assistant".to_string(),
                        content: if msg.content.is_empty() {
                            None
                        } else {
                            Some(msg.content.clone())
                        },
                        tool_calls,
                        tool_call_id: None,
                    });
                }
                Role::Tool => {
                    api_messages.push(ApiMessage {
                        role: "tool".to_string(),
                        content: Some(msg.content.clone()),
                        tool_calls: None,
                        tool_call_id: msg.tool_call_id.clone(),
                    });
                }
            }
        }

        let tools: Vec<ApiTool> = request
            .tools
            .iter()
            .map(|t| ApiTool {
                r#type: "function".to_string(),
                function: ApiFunction {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.input_schema.clone(),
                },
            })
            .collect();

        ApiRequest {
            model: request.model.clone(),
            messages: api_messages,
            max_tokens: request.max_tokens,
            tools,
        }
    }

    /// Parse API response into internal ChatResponse.
    fn parse_response(&self, api_response: ApiResponse) -> Result<ChatResponse> {
        let choice = api_response
            .choices
            .into_iter()
            .next()
            .context("Empty response from API: no choices returned")?;

        let content = choice.message.content.unwrap_or_default();

        let tool_calls = choice
            .message
            .tool_calls
            .unwrap_or_default()
            .into_iter()
            .map(|tc| ToolCall {
                id: tc.id,
                name: tc.function.name,
                arguments: tc.function.arguments,
            })
            .collect();

        Ok(ChatResponse {
            content,
            tool_calls,
        })
    }
}

#[async_trait]
impl LlmProvider for OpenAiCompatibleProvider {
    async fn chat_completion(&self, request: &ChatRequest) -> Result<ChatResponse> {
        let api_request = self.build_api_request(request);
        let url = format!(
            "{}/chat/completions",
            self.api_base.trim_end_matches('/')
        );

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&api_request)
            .send()
            .await
            .with_context(|| format!("Failed to send request to {}", url))?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            anyhow::bail!("API error ({}): {}", status, error_body);
        }

        let api_response: ApiResponse = response
            .json()
            .await
            .context("Failed to parse API response")?;

        self.parse_response(api_response)
    }

    fn name(&self) -> &str {
        "OpenAI-Compatible"
    }
}
