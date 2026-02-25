//! Read File tool implementation.
//!
//! This is the first concrete tool - it reads a file from the filesystem
//! and returns its contents. This demonstrates the complete tool lifecycle:
//!
//! 1. Tool defines its name, description, and parameter schema
//! 2. Schema is sent to the LLM as part of the request
//! 3. LLM decides to call the tool and provides arguments
//! 4. Tool executes and returns the result
//! 5. Result is sent back to the LLM as a tool_result message

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::json;

use super::Tool;

/// Tool that reads the contents of a file.
pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file at the given path. \
         Returns the full text content of the file."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        // This is a JSON Schema that tells the LLM what parameters
        // this tool accepts. The LLM will generate arguments that
        // conform to this schema.
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to read"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> Result<String> {
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .context("Missing required parameter: path")?;

        let content = tokio::fs::read_to_string(path)
            .await
            .with_context(|| format!("Failed to read file: {}", path))?;

        Ok(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Runtime::new().unwrap()
    }

    #[test]
    fn test_metadata() {
        let tool = ReadFileTool;
        assert_eq!(tool.name(), "read_file");
        assert!(!tool.description().is_empty());
        let schema = tool.parameters_schema();
        assert_eq!(schema["required"][0], "path");
    }

    #[test]
    fn test_read_existing_file() {
        let rt = rt();
        rt.block_on(async {
            let mut tmp = tempfile::NamedTempFile::new().unwrap();
            write!(tmp, "hello miniclaw").unwrap();

            let result = ReadFileTool
                .execute(json!({ "path": tmp.path().to_str().unwrap() }))
                .await
                .unwrap();

            assert_eq!(result, "hello miniclaw");
        });
    }

    #[test]
    fn test_read_nonexistent_file() {
        let rt = rt();
        rt.block_on(async {
            let result = ReadFileTool
                .execute(json!({ "path": "/tmp/__miniclaw_no_such_file__" }))
                .await;

            assert!(result.is_err());
        });
    }

    #[test]
    fn test_missing_path_param() {
        let rt = rt();
        rt.block_on(async {
            let result = ReadFileTool.execute(json!({})).await;
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("path"));
        });
    }
}
