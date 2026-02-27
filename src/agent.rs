//! Agent Loop - the core of the AI assistant.

#![allow(dead_code)]

use std::path::Path;

use anyhow::{bail, Context, Result};
use tokio::sync::mpsc;

use crate::config::{AppConfig, ModelEntry};
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
    /// Current model id for multi-model support. Used when building ChatRequest.
    current_model_id: String,
}

impl Agent {
    pub fn new(
        llm: Box<dyn LlmProvider>,
        tool_router: ToolRouter,
        config: AppConfig,
        project_root: &Path,
        current_model_id: String,
    ) -> Self {
        let model_display = config
            .get_model_entry(&current_model_id)
            .map(|m| {
                if m.name.is_empty() {
                    m.model.clone()
                } else {
                    m.name.clone()
                }
            })
            .unwrap_or_else(|| current_model_id.clone());
        let system_prompt = Self::build_system_prompt(&config, project_root, &model_display);
        let messages = vec![Message::system(&system_prompt)];
        Self {
            llm,
            tool_router,
            messages,
            config,
            stats: SessionStats::default(),
            current_model_id,
        }
    }

    /// Returns the current model id.
    pub fn current_model_id(&self) -> &str {
        &self.current_model_id
    }

    /// Returns the display name for the current model.
    pub fn current_model_display(&self) -> String {
        self.config
            .get_model_entry(&self.current_model_id)
            .map(|m| {
                if m.name.is_empty() {
                    m.model.clone()
                } else {
                    m.name.clone()
                }
            })
            .unwrap_or_else(|| self.current_model_id.clone())
    }

    /// Get the ModelEntry for the current model. Used when building ChatRequest.
    fn current_model_entry(&self) -> Option<ModelEntry> {
        self.config.get_model_entry(&self.current_model_id)
    }

    /// Return tool definitions for the given model. If model.tools is non-empty,
    /// only those tools are included; otherwise all tools from the router.
    fn tools_for_model(&self, model_entry: &ModelEntry) -> Vec<crate::types::ToolDefinition> {
        let all = self.tool_router.definitions();
        if model_entry.tools.is_empty() {
            return all;
        }
        all.into_iter()
            .filter(|d| model_entry.tools.contains(&d.name))
            .collect()
    }

    fn build_system_prompt(config: &AppConfig, project_root: &Path, model_display: &str) -> String {
        let cwd = project_root.display();
        let date = chrono::Local::now().format("%Y-%m-%d %H:%M");
        let os = std::env::consts::OS;
        let model = model_display;

        let mut prompt = format!(
            r#"You are miniclaw, an interactive terminal AI assistant for software engineering tasks.

## Environment
- Working directory: {cwd}
- Date: {date}
- OS: {os}
- Model: {model}

## Available Tools

You have access to the following tools. Use them proactively to accomplish tasks:

### read_file
Read the contents of a file. Use this to understand existing code before making changes.
- Always read a file before editing it
- For large files, read the relevant sections

### write_file
Create a new file or overwrite an existing file with complete content.
- Use for creating new files (scripts, configs, templates)
- Auto-creates parent directories
- For modifying existing files, prefer `edit` over `write_file`

### edit
Make precise text replacements in existing files.
- Provide the exact `old_text` to find (must match precisely, including whitespace)
- Only the matched text is replaced; the rest of the file is unchanged
- Safer than write_file for modifications — proves you know the current content
- Use `replace_all: true` to replace all occurrences

### bash
Execute shell commands via bash.
- Use for: building, testing, searching (grep/rg/find), git operations, installing packages
- Commands have a timeout (default 30s, configurable)
- Output is captured (stdout + stderr)
- Dangerous commands (rm, sudo, chmod) require user confirmation

### list_directory
List files and directories at a path with optional recursive traversal.

## Guidelines

1. **Read before edit**: Always read a file before modifying it to understand context
2. **Minimal changes**: Make the smallest change that accomplishes the goal
3. **Verify your work**: After making changes, use bash to run tests or verify
4. **Be concise**: Keep responses short and focused for terminal display
5. **Use Markdown**: Format output with GitHub-flavored Markdown
6. **Respond in user's language**: Match the language the user writes in
7. **Explain then act**: Briefly explain what you'll do, then do it
8. **Error handling**: If a tool call fails, explain the error and try an alternative approach

## Safety Rules
- Never execute destructive commands without user confirmation
- Do not modify files outside the working directory unless explicitly asked
- Do not guess or fabricate file contents — always read first
- If unsure about a potentially destructive action, ask the user"#
        );

        // Append user's custom system prompt from config
        let custom = config.agent.system_prompt.trim();
        if !custom.is_empty()
            && custom != "You are a helpful AI assistant. You can use tools to help the user with tasks like reading files, writing files, executing commands, and more. Be concise and helpful."
        {
            prompt.push_str(&format!("\n\n## Custom Instructions\n{}", custom));
        }

        // Append project rules (CLAUDE.md etc.)
        if let Some(rules_ctx) = rules::build_rules_context(project_root) {
            prompt.push_str(&format!(
                "\n\n## Project Rules\n<project_rules>\n{}\n</project_rules>",
                rules_ctx
            ));
        }

        prompt
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
        self.config
            .get_model_entry(&self.current_model_id)
            .map(|m| {
                if m.context_window > 0 {
                    m.context_window
                } else {
                    self.config.llm.context_window
                }
            })
            .unwrap_or(self.config.llm.context_window)
    }

    /// Truncate old messages if approaching the context window limit.
    /// Keeps the system prompt (first message) and the most recent messages.
    fn compact_context(&mut self) {
        let limit = self.context_window();
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

            let model_entry = self.current_model_entry().unwrap_or_else(|| ModelEntry {
                id: self.current_model_id.clone(),
                name: String::new(),
                provider: self.config.llm.provider.clone(),
                model: self.config.llm.model.clone(),
                api_base: self.config.llm.api_base.clone(),
                context_window: self.config.llm.context_window,
                max_tokens: self.config.llm.max_tokens,
                tools: vec![],
                enable_search: false,
                api_key: None,
                api_key_env: None,
            });

            let max_tokens = if model_entry.max_tokens > 0 {
                model_entry.max_tokens
            } else {
                self.config.llm.max_tokens
            };

            let tools = self.tools_for_model(&model_entry);
            let request = ChatRequest {
                model: model_entry.model.clone(),
                messages: self.messages.clone(),
                tools,
                max_tokens,
                enable_search: if model_entry.enable_search {
                    Some(true)
                } else {
                    None
                },
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
        Self::create_with_model(config, project_root, None)
    }

    /// Create Agent with a specific model. Pass None to use config default.
    pub fn create_with_model(
        config: &AppConfig,
        project_root: &Path,
        model_id: Option<&str>,
    ) -> Result<Self> {
        let model_id = model_id
            .map(String::from)
            .unwrap_or_else(|| config.default_model_id());
        let api_key = config.api_key_for_model(&model_id)?;
        let entry = config
            .get_model_entry(&model_id)
            .unwrap_or_else(|| ModelEntry {
                id: model_id.clone(),
                name: String::new(),
                provider: config.llm.provider.clone(),
                model: config.llm.model.clone(),
                api_base: config.llm.api_base.clone(),
                context_window: config.llm.context_window,
                max_tokens: config.llm.max_tokens,
                tools: vec![],
                enable_search: false,
                api_key: None,
                api_key_env: None,
            });
        let llm = Self::create_provider_for_model(&api_key, &entry)?;
        let tool_router = create_default_router();
        Ok(Self::new(
            llm,
            tool_router,
            config.clone(),
            project_root,
            model_id,
        ))
    }

    fn create_provider_for_model(
        api_key: &str,
        entry: &ModelEntry,
    ) -> Result<Box<dyn LlmProvider>> {
        let llm: Box<dyn LlmProvider> = match entry.provider.as_str() {
            "anthropic" => Box::new(AnthropicProvider::new(
                api_key.to_string(),
                entry.api_base.clone(),
            )),
            "openai_compatible" | "openai" => Box::new(OpenAiCompatibleProvider::new(
                api_key.to_string(),
                entry.api_base.clone(),
            )),
            other => bail!(
                "Unknown provider: '{}'. Supported: 'anthropic', 'openai_compatible'",
                other
            ),
        };
        Ok(llm)
    }

    /// Switch to a different model. Recreates the LLM provider.
    pub fn switch_model(&mut self, model_id: &str, config: &AppConfig) -> Result<()> {
        let entry = config.get_model_entry(model_id).with_context(|| {
            format!(
                "Model '{}' not found. Use /model to list available models.",
                model_id
            )
        })?;
        let api_key = config.api_key_for_model(model_id)?;
        let llm = Self::create_provider_for_model(&api_key, &entry)?;
        self.llm = llm;
        self.current_model_id = model_id.to_string();
        Ok(())
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
