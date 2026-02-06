//! Tool System module.
//!
//! This module defines the `Tool` trait and `ToolRouter` that together
//! form the tool execution framework.
//!
//! Key concepts:
//! - **Tool trait**: every tool implements this trait, providing its name,
//!   description, JSON Schema for parameters, and an execute method
//! - **JSON Schema**: a standard way to describe data structures. The LLM
//!   reads the schema to know what arguments a tool expects.
//! - **ToolRouter**: a registry that holds all available tools and dispatches
//!   tool calls by name to the correct implementation
//! - **Box<dyn Tool>**: Rust's way of storing different types that implement
//!   the same trait in a single collection (trait objects / dynamic dispatch)

pub mod read_file;

use anyhow::{Context, Result};
use async_trait::async_trait;

use crate::types::ToolDefinition;

/// Trait that all tools must implement.
///
/// Each tool is a capability that the LLM can invoke.
/// Tools receive JSON arguments and return a string result.
#[async_trait]
pub trait Tool: Send + Sync {
    /// The unique name of this tool (e.g. "read_file").
    fn name(&self) -> &str;

    /// A human-readable description of what this tool does.
    /// The LLM reads this to decide when to use the tool.
    fn description(&self) -> &str;

    /// JSON Schema describing the tool's input parameters.
    /// The LLM uses this to generate valid arguments.
    fn parameters_schema(&self) -> serde_json::Value;

    /// Execute the tool with the given JSON arguments.
    /// Returns a string result that will be sent back to the LLM.
    async fn execute(&self, params: serde_json::Value) -> Result<String>;

    /// Convert this tool into a ToolDefinition for sending to the LLM.
    fn to_definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            input_schema: self.parameters_schema(),
        }
    }
}

/// Routes tool calls to the correct tool implementation.
///
/// The ToolRouter holds a collection of registered tools and
/// can dispatch execution requests by tool name.
pub struct ToolRouter {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRouter {
    /// Create a new empty ToolRouter.
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    /// Register a tool with the router.
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.push(tool);
    }

    /// Get all tool definitions (for sending to the LLM).
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.iter().map(|t| t.to_definition()).collect()
    }

    /// Execute a tool by name with the given arguments.
    pub async fn execute(&self, name: &str, arguments: &str) -> Result<String> {
        let tool = self
            .tools
            .iter()
            .find(|t| t.name() == name)
            .with_context(|| format!("Unknown tool: {}", name))?;

        let params: serde_json::Value = serde_json::from_str(arguments)
            .with_context(|| format!("Invalid JSON arguments for tool '{}': {}", name, arguments))?;

        tool.execute(params).await
    }

    /// Check if a tool with the given name is registered.
    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.iter().any(|t| t.name() == name)
    }

    /// Get the number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Check if the router has no tools.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

impl Default for ToolRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a ToolRouter with all built-in tools registered.
pub fn create_default_router() -> ToolRouter {
    let mut router = ToolRouter::new();
    router.register(Box::new(read_file::ReadFileTool));
    // More tools will be added here in Phase 5:
    // router.register(Box::new(write_file::WriteFileTool));
    // router.register(Box::new(exec_command::ExecCommandTool));
    // router.register(Box::new(list_dir::ListDirTool));
    router
}
