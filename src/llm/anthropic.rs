//! Anthropic (Claude) LLM provider implementation.

use anyhow::{Context, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use super::LlmProvider;
use crate::types::{ChatRequest, ChatResponse, Role, StreamChunk, TokenUsage, ToolCall};

pub struct AnthropicProvider {
    api_key: String,
    api_base: String,
    client: reqwest::Client,
}

// --- API Request Types ---

#[derive(Serialize)]
struct ApiRequest {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ApiTool>,
}

#[derive(Serialize)]
struct ApiMessage {
    role: String,
    content: ApiContent,
}

#[derive(Serialize)]
#[serde(untagged)]
enum ApiContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

#[derive(Serialize)]
struct ApiTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

// --- API Response Types ---

#[derive(Deserialize, Debug)]
struct ApiResponse {
    content: Vec<ContentBlock>,
    #[allow(dead_code)]
    stop_reason: Option<String>,
    usage: Option<ApiUsage>,
}

#[derive(Deserialize, Debug)]
struct ApiUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
}

// --- Implementation ---

impl AnthropicProvider {
    pub fn new(api_key: String, api_base: Option<String>) -> Self {
        Self {
            api_key,
            api_base: api_base.unwrap_or_else(|| "https://api.anthropic.com".to_string()),
            client: reqwest::Client::new(),
        }
    }

    fn build_api_request(&self, request: &ChatRequest) -> ApiRequest {
        let mut system = None;
        let mut api_messages: Vec<ApiMessage> = Vec::new();

        for msg in &request.messages {
            match msg.role {
                Role::System => {
                    system = Some(msg.content.clone());
                }
                Role::User => {
                    api_messages.push(ApiMessage {
                        role: "user".to_string(),
                        content: ApiContent::Text(msg.content.clone()),
                    });
                }
                Role::Assistant => {
                    if msg.tool_calls.is_empty() {
                        api_messages.push(ApiMessage {
                            role: "assistant".to_string(),
                            content: ApiContent::Text(msg.content.clone()),
                        });
                    } else {
                        let mut blocks = Vec::new();
                        if !msg.content.is_empty() {
                            blocks.push(ContentBlock::Text {
                                text: msg.content.clone(),
                            });
                        }
                        for tc in &msg.tool_calls {
                            let input: serde_json::Value =
                                serde_json::from_str(&tc.arguments).unwrap_or_default();
                            blocks.push(ContentBlock::ToolUse {
                                id: tc.id.clone(),
                                name: tc.name.clone(),
                                input,
                            });
                        }
                        api_messages.push(ApiMessage {
                            role: "assistant".to_string(),
                            content: ApiContent::Blocks(blocks),
                        });
                    }
                }
                Role::Tool => {
                    let block = ContentBlock::ToolResult {
                        tool_use_id: msg.tool_call_id.clone().unwrap_or_default(),
                        content: msg.content.clone(),
                    };
                    api_messages.push(ApiMessage {
                        role: "user".to_string(),
                        content: ApiContent::Blocks(vec![block]),
                    });
                }
            }
        }

        let tools: Vec<ApiTool> = request
            .tools
            .iter()
            .map(|t| ApiTool {
                name: t.name.clone(),
                description: t.description.clone(),
                input_schema: t.input_schema.clone(),
            })
            .collect();

        ApiRequest {
            model: request.model.clone(),
            max_tokens: request.max_tokens,
            system,
            messages: api_messages,
            tools,
        }
    }

    fn parse_response(&self, api_response: ApiResponse) -> ChatResponse {
        let mut content = String::new();
        let mut tool_calls = Vec::new();

        for block in api_response.content {
            match block {
                ContentBlock::Text { text } => content.push_str(&text),
                ContentBlock::ToolUse { id, name, input } => {
                    tool_calls.push(ToolCall {
                        id,
                        name,
                        arguments: serde_json::to_string(&input).unwrap_or_default(),
                    });
                }
                ContentBlock::ToolResult { .. } => {}
            }
        }

        let usage = api_response.usage.map(|u| TokenUsage {
            input_tokens: u.input_tokens.unwrap_or(0),
            output_tokens: u.output_tokens.unwrap_or(0),
        });

        ChatResponse {
            content,
            tool_calls,
            usage,
        }
    }
}

#[derive(Default)]
struct StreamToolCallAccumulator {
    id: String,
    name: String,
    arguments: String,
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    async fn chat_completion(&self, request: &ChatRequest) -> Result<ChatResponse> {
        let api_request = self.build_api_request(request);
        let url = format!("{}/v1/messages", self.api_base.trim_end_matches('/'));

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&api_request)
            .send()
            .await
            .context("Failed to send request to Anthropic API")?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API error ({}): {}", status, error_body);
        }

        let api_response: ApiResponse = response
            .json()
            .await
            .context("Failed to parse Anthropic API response")?;

        Ok(self.parse_response(api_response))
    }

    async fn chat_completion_stream(
        &self,
        request: &ChatRequest,
        chunk_tx: mpsc::UnboundedSender<StreamChunk>,
    ) -> Result<ChatResponse> {
        let api_request = self.build_api_request(request);
        let url = format!("{}/v1/messages", self.api_base.trim_end_matches('/'));

        let mut body = serde_json::to_value(&api_request).context("Failed to serialize request")?;
        body["stream"] = serde_json::json!(true);

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to send streaming request to Anthropic API")?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API error ({}): {}", status, error_body);
        }

        let mut byte_stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut content = String::new();
        let mut tool_calls: Vec<StreamToolCallAccumulator> = Vec::new();
        let mut input_tokens: u64 = 0;
        let mut output_tokens: u64 = 0;
        let mut current_event_type = String::new();

        while let Some(chunk_result) = byte_stream.next().await {
            let chunk_bytes = chunk_result.context("Stream read error")?;
            buffer.push_str(&String::from_utf8_lossy(&chunk_bytes));

            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim_end_matches('\r').to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                if line.is_empty() {
                    current_event_type.clear();
                    continue;
                }

                if let Some(event_type) = line.strip_prefix("event: ") {
                    current_event_type = event_type.trim().to_string();
                    continue;
                }

                let data = match line.strip_prefix("data: ") {
                    Some(d) => d,
                    None => continue,
                };

                let Ok(v) = serde_json::from_str::<serde_json::Value>(data) else {
                    continue;
                };

                match current_event_type.as_str() {
                    "message_start" => {
                        if let Some(u) = v.get("message").and_then(|m| m.get("usage")) {
                            input_tokens =
                                u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                        }
                    }
                    "content_block_start" => {
                        if let Some(block) = v.get("content_block") {
                            let block_type =
                                block.get("type").and_then(|v| v.as_str()).unwrap_or("");
                            if block_type == "tool_use" {
                                let id = block
                                    .get("id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let name = block
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                tool_calls.push(StreamToolCallAccumulator {
                                    id,
                                    name,
                                    arguments: String::new(),
                                });
                            }
                        }
                    }
                    "content_block_delta" => {
                        if let Some(delta) = v.get("delta") {
                            let delta_type =
                                delta.get("type").and_then(|v| v.as_str()).unwrap_or("");
                            match delta_type {
                                "text_delta" => {
                                    if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                                        content.push_str(text);
                                        let _ =
                                            chunk_tx.send(StreamChunk::TextDelta(text.to_string()));
                                    }
                                }
                                "input_json_delta" => {
                                    if let Some(json) =
                                        delta.get("partial_json").and_then(|v| v.as_str())
                                    {
                                        if let Some(tc) = tool_calls.last_mut() {
                                            tc.arguments.push_str(json);
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    "message_delta" => {
                        if let Some(u) = v.get("usage") {
                            output_tokens =
                                u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                        }
                    }
                    "message_stop" => {
                        let _ = chunk_tx.send(StreamChunk::Done);
                    }
                    _ => {}
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

        let usage = if input_tokens > 0 || output_tokens > 0 {
            Some(TokenUsage {
                input_tokens,
                output_tokens,
            })
        } else {
            None
        };

        Ok(ChatResponse {
            content,
            tool_calls: final_tool_calls,
            usage,
        })
    }

    fn name(&self) -> &str {
        "Anthropic"
    }
}
