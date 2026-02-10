//! Write File tool implementation.
//!
//! This tool allows the AI assistant to write content to a file.

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::json;

use super::Tool;

/// Tool that writes content to a file.
pub struct WriteFileTool;

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file at the given path. \
         Creates the file if it doesn't exist, overwrites if it does."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        // JSON Schema that describes the parameters for this tool
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file"
                }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> Result<String> {
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .context("Missing required parameter: path")?;

        let content = params
            .get("content")
            .and_then(|v| v.as_str())
            .context("Missing required parameter: content")?;

        // Create directory if it doesn't exist
        if let Some(parent) = std::path::Path::new(path).parent() {
            tokio::fs::create_dir_all(parent).await
                .with_context(|| format!("Failed to create directory for: {}", path))?;
        }

        // Write the file
        tokio::fs::write(path, content)
            .await
            .with_context(|| format!("Failed to write file: {}", path))?;

        Ok(format!("Successfully wrote {} characters to file: {}", content.len(), path))
    }
}