//! Agent Loop - the core of the AI assistant.

#![allow(dead_code)]

use std::path::Path;

use anyhow::{bail, Context, Result};
use tokio::sync::mpsc;

use crate::config::AppConfig;
use crate::llm::anthropic::AnthropicProvider;
use crate::llm::openai_compatible::OpenAiCompatibleProvider;
use crate::llm::LlmProvider;
use crate::rules;
use crate::tools::risk::{self, RiskLevel};
use crate::tools::{create_default_router, ToolRouter};
use crate::types::{ChatRequest, ChatResponse, Message, StreamChunk, TokenUsage};

/// Events emitted by the Agent during processing, allowing the TUI
/// to display real-time progress (tool calls, intermediate text, etc.).
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum AgentEvent {
    /// Incremental text chunk from streaming LLM response.
    StreamDelta(String),
    /// Intermediate text from LLM emitted alongside tool_calls (non-streaming fallback).
    LlmText(String),
    /// A tool is about to be executed.
    ToolStart { name: String, arguments: String },
    /// A tool finished executing.
    ToolEnd {
        name: String,
        arguments: String,
        success: bool,
    },
    /// A dangerous tool call needs user confirmation before execution.
    ToolConfirm {
        name: String,
        arguments: String,
        description: String,
    },
    /// Final response ready (content may be empty if already streamed).
    Done(String),
    /// An error occurred.
    Error(String),
}

/// Cumulative usage statistics tracked across the session.
#[derive(Debug, Clone, Default)]
pub struct SessionStats {
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub request_count: u64,
}

impl SessionStats {
    fn record_usage(&mut self, usage: &Option<TokenUsage>) {
        if let Some(u) = usage {
            self.total_input_tokens += u.input_tokens;
            self.total_output_tokens += u.output_tokens;
        }
        self.request_count += 1;
    }
}

pub struct Agent {
    llm: Box<dyn LlmProvider>,
    tool_router: ToolRouter,
    messages: Vec<Message>,
    config: AppConfig,
    pub stats: SessionStats,
}

impl Agent {
    pub fn new(
        llm: Box<dyn LlmProvider>,
        tool_router: ToolRouter,
        config: AppConfig,
        project_root: &Path,
    ) -> Self {
        let system_prompt = Self::build_system_prompt(&config, project_root);
        let messages = vec![Message::system(&system_prompt)];
        Self {
            llm,
            tool_router,
            messages,
            config,
            stats: SessionStats::default(),
        }
    }

    fn build_system_prompt(config: &AppConfig, project_root: &Path) -> String {
        let base = &config.agent.system_prompt;
        match rules::build_rules_context(project_root) {
            Some(rules_ctx) => format!(
                "{}\n\n<project_rules>\n{}\n</project_rules>",
                base, rules_ctx
            ),
            None => base.clone(),
        }
    }

    /// Rough token estimation: ~4 chars per token for English, ~2 for CJK.
    fn estimate_tokens(text: &str) -> u64 {
        let char_count = text.chars().count() as u64;
        (char_count / 3).max(1)
    }

    /// Estimate total tokens across all messages.
    pub fn estimate_context_tokens(&self) -> u64 {
        self.messages
            .iter()
            .map(|m| {
                let content_tokens = Self::estimate_tokens(&m.content);
                let tool_tokens: u64 = m
                    .tool_calls
                    .iter()
                    .map(|tc| Self::estimate_tokens(&tc.arguments) + 10)
                    .sum();
                content_tokens + tool_tokens + 4 // overhead per message
            })
            .sum()
    }

    pub fn context_window(&self) -> u64 {
        self.config.llm.context_window
    }

    /// Truncate old messages if approaching the context window limit.
    /// Keeps the system prompt (first message) and the most recent messages.
    fn compact_context(&mut self) {
        let limit = self.config.llm.context_window;
        let threshold = (limit as f64 * 0.85) as u64;

        if self.estimate_context_tokens() <= threshold {
            return;
        }

        // Keep system prompt (index 0) and remove oldest non-system messages
        while self.messages.len() > 2 && self.estimate_context_tokens() > threshold {
            self.messages.remove(1);
        }
    }

    pub async fn process_message(
        &mut self,
        user_input: &str,
        event_tx: Option<mpsc::UnboundedSender<AgentEvent>>,
        mut confirm_rx: Option<&mut mpsc::UnboundedReceiver<bool>>,
    ) -> Result<String> {
        self.messages.push(Message::user(user_input));
        self.compact_context();

        let emit = |evt: AgentEvent| {
            if let Some(tx) = &event_tx {
                let _ = tx.send(evt);
            }
        };

        let mut iterations = 0;
        let max_iterations = self.config.agent.max_iterations;

        loop {
            iterations += 1;
            if iterations > max_iterations {
                let msg = format!(
                    "[Agent stopped: reached maximum of {} iterations]",
                    max_iterations
                );
                emit(AgentEvent::Done(msg.clone()));
                return Ok(msg);
            }

            let request = ChatRequest {
                model: self.config.llm.model.clone(),
                messages: self.messages.clone(),
                tools: self.tool_router.definitions(),
                max_tokens: self.config.llm.max_tokens,
            };

            let (chunk_tx, mut chunk_rx) = mpsc::unbounded_channel::<StreamChunk>();

            let event_tx_clone = event_tx.clone();
            let forward_handle = tokio::spawn(async move {
                while let Some(chunk) = chunk_rx.recv().await {
                    if let StreamChunk::TextDelta(delta) = chunk {
                        if let Some(tx) = &event_tx_clone {
                            let _ = tx.send(AgentEvent::StreamDelta(delta));
                        }
                    }
                }
            });

            let response: ChatResponse = self
                .llm
                .chat_completion_stream(&request, chunk_tx)
                .await
                .context("LLM streaming call failed")?;

            let _ = forward_handle.await;

            self.stats.record_usage(&response.usage);

            if response.has_tool_calls() {
                self.messages.push(Message::assistant_with_tool_calls(
                    &response.content,
                    response.tool_calls.clone(),
                ));

                for tool_call in &response.tool_calls {
                    let risk = risk::assess_risk(&tool_call.name, &tool_call.arguments);

                    if risk == RiskLevel::Dangerous {
                        let desc = risk::describe_tool_call(&tool_call.name, &tool_call.arguments);
                        emit(AgentEvent::ToolConfirm {
                            name: tool_call.name.clone(),
                            arguments: tool_call.arguments.clone(),
                            description: desc,
                        });

                        let approved = if let Some(rx) = confirm_rx.as_mut() {
                            rx.recv().await.unwrap_or(false)
                        } else {
                            false
                        };

                        if !approved {
                            let deny_msg =
                                format!("Tool call '{}' was denied by the user.", tool_call.name);
                            emit(AgentEvent::ToolEnd {
                                name: tool_call.name.clone(),
                                arguments: tool_call.arguments.clone(),
                                success: false,
                            });
                            self.messages
                                .push(Message::tool_result(&tool_call.id, &deny_msg));
                            continue;
                        }
                    }

                    emit(AgentEvent::ToolStart {
                        name: tool_call.name.clone(),
                        arguments: tool_call.arguments.clone(),
                    });

                    let result = self
                        .tool_router
                        .execute(&tool_call.name, &tool_call.arguments)
                        .await;

                    let (result_text, success) = match result {
                        Ok(output) => (output, true),
                        Err(e) => (format!("Error: {}", e), false),
                    };

                    emit(AgentEvent::ToolEnd {
                        name: tool_call.name.clone(),
                        arguments: tool_call.arguments.clone(),
                        success,
                    });

                    self.messages
                        .push(Message::tool_result(&tool_call.id, &result_text));
                }
                continue;
            }

            self.messages.push(Message::assistant(&response.content));
            emit(AgentEvent::Done(response.content.clone()));
            return Ok(response.content);
        }
    }

    /// Factory method: create a new Agent from config (creates LLM provider + tool router).
    pub fn create(config: &AppConfig, project_root: &Path) -> Result<Self> {
        let api_key = config.api_key()?;
        let api_base = config.llm.api_base.clone();
        let llm: Box<dyn LlmProvider> = match config.llm.provider.as_str() {
            "anthropic" => Box::new(AnthropicProvider::new(api_key, api_base)),
            "openai_compatible" | "openai" => {
                Box::new(OpenAiCompatibleProvider::new(api_key, api_base))
            }
            other => bail!(
                "Unknown provider: '{}'. Supported: 'anthropic', 'openai_compatible'",
                other
            ),
        };
        let tool_router = create_default_router();
        Ok(Self::new(llm, tool_router, config.clone(), project_root))
    }

    pub fn history(&self) -> &[Message] {
        &self.messages
    }

    /// Replace the message history (used when restoring a saved session).
    pub fn set_messages(&mut self, messages: Vec<Message>) {
        self.messages = messages;
    }

    pub fn clear_history(&mut self) {
        self.messages.truncate(1);
    }
}
