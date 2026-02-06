//! Agent Loop - the core of the AI assistant.
//!
//! The Agent orchestrates the conversation between the user, the LLM,
//! and the tools. It implements the fundamental "agent loop" pattern:
//!
//! ```text
//! User Input
//!     |
//!     v
//! +--------+     +-----+     +-------+
//! |  LLM   |<--->|Agent|<--->| Tools |
//! +--------+     +-----+     +-------+
//!     |              |
//!     v              v
//! Text Reply    Tool Results
//! ```
//!
//! The loop continues until the LLM returns a text response
//! (no more tool calls) or the max iteration limit is reached.

use anyhow::{Context, Result};

use crate::config::AppConfig;
use crate::llm::LlmProvider;
use crate::tools::ToolRouter;
use crate::types::{ChatRequest, ChatResponse, Message};

/// The Agent holds all components and manages the conversation.
pub struct Agent {
    /// The LLM provider (Anthropic, OpenAI, etc.)
    llm: Box<dyn LlmProvider>,
    /// The tool router (dispatches tool calls)
    tool_router: ToolRouter,
    /// Conversation history
    messages: Vec<Message>,
    /// Configuration
    config: AppConfig,
}

impl Agent {
    /// Create a new Agent with the given components.
    pub fn new(
        llm: Box<dyn LlmProvider>,
        tool_router: ToolRouter,
        config: AppConfig,
    ) -> Self {
        let mut messages = Vec::new();
        // Inject system prompt as the first message
        messages.push(Message::system(&config.agent.system_prompt));

        Self {
            llm,
            tool_router,
            messages,
            config,
        }
    }

    /// Process a user message through the agent loop.
    ///
    /// This is the core method. It:
    /// 1. Adds the user message to history
    /// 2. Calls the LLM
    /// 3. If LLM wants to use tools -> execute them -> feed results back -> repeat
    /// 4. Returns the final text response
    pub async fn process_message(&mut self, user_input: &str) -> Result<String> {
        // Step 1: Add user message to history
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

            // Step 2: Build request and call LLM
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

            // Step 3: Check if LLM wants to call tools
            if response.has_tool_calls() {
                // Store the assistant message with tool calls in history
                self.messages.push(Message::assistant_with_tool_calls(
                    &response.content,
                    response.tool_calls.clone(),
                ));

                // Execute each tool call
                for tool_call in &response.tool_calls {
                    println!("  [Tool: {} ...]", tool_call.name);

                    let result = self
                        .tool_router
                        .execute(&tool_call.name, &tool_call.arguments)
                        .await;

                    let result_text = match result {
                        Ok(output) => output,
                        Err(e) => format!("Error: {}", e),
                    };

                    // Add tool result to history
                    self.messages
                        .push(Message::tool_result(&tool_call.id, &result_text));
                }

                // Continue the loop - LLM will see the tool results
                continue;
            }

            // Step 4: LLM returned a text response (no tool calls) -> done
            self.messages.push(Message::assistant(&response.content));
            return Ok(response.content);
        }
    }

    /// Get a reference to the conversation history.
    pub fn history(&self) -> &[Message] {
        &self.messages
    }

    /// Clear the conversation history (keeps system prompt).
    pub fn clear_history(&mut self) {
        self.messages.truncate(1); // Keep the system message
    }
}
