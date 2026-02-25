//! OpenAI-compatible LLM provider implementation.

use anyhow::{Context, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use super::LlmProvider;
use crate::types::{ChatRequest, ChatResponse, Role, StreamChunk, ToolCall, TokenUsage};

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
    usage: Option<ApiUsage>,
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

#[derive(Deserialize, Debug)]
struct ApiUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
}

// --- Streaming Response Types ---

#[derive(Deserialize, Debug)]
struct StreamResponseChunk {
    choices: Vec<StreamChoice>,
    usage: Option<ApiUsage>,
}

#[derive(Deserialize, Debug)]
struct StreamChoice {
    delta: StreamDelta,
}

#[derive(Deserialize, Debug)]
struct StreamDelta {
    content: Option<String>,
    tool_calls: Option<Vec<StreamToolCallDelta>>,
}

#[derive(Deserialize, Debug)]
struct StreamToolCallDelta {
    index: usize,
    id: Option<String>,
    function: Option<StreamFunctionDelta>,
}

#[derive(Deserialize, Debug)]
struct StreamFunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Default)]
struct ToolCallAccumulator {
    id: String,
    name: String,
    arguments: String,
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
                        content: if msg.content.is_empty() { None } else { Some(msg.content.clone()) },
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

        let usage = api_response.usage.map(|u| TokenUsage {
            input_tokens: u.prompt_tokens.unwrap_or(0),
            output_tokens: u.completion_tokens.unwrap_or(0),
        });

        Ok(ChatResponse { content, tool_calls, usage })
    }
}

#[async_trait]
impl LlmProvider for OpenAiCompatibleProvider {
    async fn chat_completion(&self, request: &ChatRequest) -> Result<ChatResponse> {
        let api_request = self.build_api_request(request);
        let url = format!("{}/chat/completions", self.api_base.trim_end_matches('/'));

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

    async fn chat_completion_stream(
        &self,
        request: &ChatRequest,
        chunk_tx: mpsc::UnboundedSender<StreamChunk>,
    ) -> Result<ChatResponse> {
        let api_request = self.build_api_request(request);
        let url = format!("{}/chat/completions", self.api_base.trim_end_matches('/'));

        let mut body = serde_json::to_value(&api_request)
            .context("Failed to serialize request")?;
        body["stream"] = serde_json::json!(true);
        body["stream_options"] = serde_json::json!({"include_usage": true});

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .with_context(|| format!("Failed to send streaming request to {}", url))?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            anyhow::bail!("API error ({}): {}", status, error_body);
        }

        let mut byte_stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut content = String::new();
        let mut tool_calls: Vec<ToolCallAccumulator> = Vec::new();
        let mut usage: Option<TokenUsage> = None;

        while let Some(chunk_result) = byte_stream.next().await {
            let chunk_bytes = chunk_result.context("Stream read error")?;
            buffer.push_str(&String::from_utf8_lossy(&chunk_bytes));

            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim_end_matches('\r').to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                if line.is_empty() {
                    continue;
                }

                let data = match line.strip_prefix("data: ") {
                    Some(d) => d,
                    None => continue,
                };

                if data.trim() == "[DONE]" {
                    let _ = chunk_tx.send(StreamChunk::Done);
                    let final_tool_calls = tool_calls
                        .into_iter()
                        .map(|tc| ToolCall {
                            id: tc.id,
                            name: tc.name,
                            arguments: tc.arguments,
                        })
                        .collect();
                    return Ok(ChatResponse {
                        content,
                        tool_calls: final_tool_calls,
                        usage,
                    });
                }

                if let Ok(chunk_resp) = serde_json::from_str::<StreamResponseChunk>(data) {
                    if let Some(choice) = chunk_resp.choices.first() {
                        if let Some(ref text) = choice.delta.content {
                            if !text.is_empty() {
                                content.push_str(text);
                                let _ = chunk_tx.send(StreamChunk::TextDelta(text.clone()));
                            }
                        }
                        if let Some(ref tcs) = choice.delta.tool_calls {
                            for tc_delta in tcs {
                                while tool_calls.len() <= tc_delta.index {
                                    tool_calls.push(ToolCallAccumulator::default());
                                }
                                let acc = &mut tool_calls[tc_delta.index];
                                if let Some(ref id) = tc_delta.id {
                                    acc.id = id.clone();
                                }
                                if let Some(ref func) = tc_delta.function {
                                    if let Some(ref name) = func.name {
                                        acc.name.push_str(name);
                                    }
                                    if let Some(ref args) = func.arguments {
                                        acc.arguments.push_str(args);
                                    }
                                }
                            }
                        }
                    }
                    if let Some(u) = chunk_resp.usage {
                        usage = Some(TokenUsage {
                            input_tokens: u.prompt_tokens.unwrap_or(0),
                            output_tokens: u.completion_tokens.unwrap_or(0),
                        });
                    }
                }
            }
        }

        let _ = chunk_tx.send(StreamChunk::Done);
        let final_tool_calls = tool_calls
            .into_iter()
            .map(|tc| ToolCall {
                id: tc.id,
                name: tc.name,
                arguments: tc.arguments,
            })
            .collect();
        Ok(ChatResponse {
            content,
            tool_calls: final_tool_calls,
            usage,
        })
    }

    fn name(&self) -> &str {
        "OpenAI-Compatible"
    }
}
