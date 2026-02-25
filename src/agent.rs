//! Agent Loop - the core of the AI assistant.

use std::path::Path;

use anyhow::{Context, Result};

use crate::config::AppConfig;
use crate::llm::LlmProvider;
use crate::rules;
use crate::tools::ToolRouter;
use crate::types::{ChatRequest, ChatResponse, Message, TokenUsage};

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
        let mut messages = Vec::new();
        messages.push(Message::system(&system_prompt));
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

    pub async fn process_message(&mut self, user_input: &str) -> Result<String> {
        self.messages.push(Message::user(user_input));

        let mut iterations = 0;
        let max_iterations = self.config.agent.max_iterations;

        loop {
            iterations += 1;
            if iterations > max_iterations {
                return Ok(format!(
                    "[Agent stopped: reached maximum of {} iterations]",
                    max_iterations
                ));
            }

            let request = ChatRequest {
                model: self.config.llm.model.clone(),
                messages: self.messages.clone(),
                tools: self.tool_router.definitions(),
                max_tokens: self.config.llm.max_tokens,
            };

            let response: ChatResponse = self
                .llm
                .chat_completion(&request)
                .await
                .context("LLM call failed")?;

            self.stats.record_usage(&response.usage);

            if response.has_tool_calls() {
                self.messages.push(Message::assistant_with_tool_calls(
                    &response.content,
                    response.tool_calls.clone(),
                ));

                for tool_call in &response.tool_calls {
                    let result = self
                        .tool_router
                        .execute(&tool_call.name, &tool_call.arguments)
                        .await;
                    let result_text = match result {
                        Ok(output) => output,
                        Err(e) => format!("Error: {}", e),
                    };
                    self.messages
                        .push(Message::tool_result(&tool_call.id, &result_text));
                }
                continue;
            }

            self.messages.push(Message::assistant(&response.content));
            return Ok(response.content);
        }
    }

    pub fn history(&self) -> &[Message] {
        &self.messages
    }

    pub fn clear_history(&mut self) {
        self.messages.truncate(1);
    }
}
