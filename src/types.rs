//! Core data types used throughout miniclaw.
//!
//! This module defines the message types, tool call structures,
//! and request/response formats that flow between all components.

use serde::{Deserialize, Serialize};

// --- Message Roles ---

/// The role of a message in the conversation.
///
/// LLM APIs use roles to distinguish who said what:
/// - `System`: instructions to the AI (invisible to the user)
/// - `User`: the human's input
/// - `Assistant`: the AI's response
/// - `Tool`: the result of a tool execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

// --- Tool Call ---

/// Represents a tool call request from the LLM.
///
/// When the LLM decides it needs to use a tool, it returns a ToolCall
/// containing the tool's name and the arguments (as a JSON string).
/// The `id` is used to match the tool result back to the request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique identifier for this tool call (used to match results)
    pub id: String,
    /// Name of the tool to invoke (e.g. "read_file")
    pub name: String,
    /// JSON-encoded arguments for the tool
    pub arguments: String,
}

// --- Tool Definition ---

/// Describes a tool's interface to the LLM via JSON Schema.
///
/// This is sent to the LLM so it knows what tools are available,
/// what each tool does, and what parameters it accepts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// The tool's name (must match what the tool reports)
    pub name: String,
    /// Human-readable description of what the tool does
    pub description: String,
    /// JSON Schema describing the tool's input parameters
    pub input_schema: serde_json::Value,
}

// --- Messages ---

/// A single message in the conversation history.
///
/// Messages flow between user, assistant, and tool results.
/// The conversation is modeled as a `Vec<Message>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    /// If the assistant wants to call tools, this will be non-empty
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    /// For tool result messages, this links back to the tool call ID
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl Message {
    /// Create a system message (sets the AI's behavior/instructions).
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
            tool_calls: vec![],
            tool_call_id: None,
        }
    }

    /// Create a user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            tool_calls: vec![],
            tool_call_id: None,
        }
    }

    /// Create an assistant message (text reply from the AI).
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            tool_calls: vec![],
            tool_call_id: None,
        }
    }

    /// Create an assistant message that includes tool calls.
    pub fn assistant_with_tool_calls(
        content: impl Into<String>,
        tool_calls: Vec<ToolCall>,
    ) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            tool_calls,
            tool_call_id: None,
        }
    }

    /// Create a tool result message.
    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: content.into(),
            tool_calls: vec![],
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

// --- Chat Request / Response ---

/// A request to send to the LLM.
///
/// This is our internal representation; the LLM client will convert
/// this into the provider-specific API format.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    /// The model to use (e.g. "claude-sonnet-4-20250514")
    pub model: String,
    /// The conversation messages
    pub messages: Vec<Message>,
    /// Available tools for the LLM to call
    pub tools: Vec<ToolDefinition>,
    /// Maximum tokens in the response
    pub max_tokens: u32,
}

/// The response from an LLM call.
///
/// Contains either a text reply, tool calls, or both.
#[derive(Debug, Clone)]
pub struct ChatResponse {
    /// The text content of the response (may be empty if only tool calls)
    pub content: String,
    /// Tool calls the LLM wants to make (empty if just a text reply)
    pub tool_calls: Vec<ToolCall>,
}

impl ChatResponse {
    /// Returns true if the LLM wants to call tools.
    pub fn has_tool_calls(&self) -> bool {
        !self.tool_calls.is_empty()
    }
}

// --- Stream Chunk ---

/// A single chunk from a streaming LLM response.
///
/// When streaming, the response comes in small pieces.
/// Each chunk is either a text delta or indicates completion.
#[derive(Debug, Clone)]
pub enum StreamChunk {
    /// A piece of text content
    TextDelta(String),
    /// The stream is complete
    Done,
}
