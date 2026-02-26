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
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("Failed to create directory for: {}", path))?;
        }

        // Write the file
        tokio::fs::write(path, content)
            .await
            .with_context(|| format!("Failed to write file: {}", path))?;

        Ok(format!(
            "Successfully wrote {} characters to file: {}",
            content.len(),
            path
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Runtime::new().unwrap()
    }

    #[test]
    fn test_metadata() {
        let tool = WriteFileTool;
        assert_eq!(tool.name(), "write_file");
        assert!(!tool.description().is_empty());
        let schema = tool.parameters_schema();
        let required = schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "path"));
        assert!(required.iter().any(|v| v == "content"));
    }

    #[test]
    fn test_write_new_file() {
        let rt = rt();
        rt.block_on(async {
            let dir = tempfile::tempdir().unwrap();
            let file_path = dir.path().join("test.txt");

            let result = WriteFileTool
                .execute(json!({
                    "path": file_path.to_str().unwrap(),
                    "content": "hello world"
                }))
                .await
                .unwrap();

            assert!(result.contains("11 characters"));
            assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "hello world");
        });
    }

    #[test]
    fn test_write_creates_parent_dirs() {
        let rt = rt();
        rt.block_on(async {
            let dir = tempfile::tempdir().unwrap();
            let file_path = dir.path().join("sub").join("deep").join("file.txt");

            WriteFileTool
                .execute(json!({
                    "path": file_path.to_str().unwrap(),
                    "content": "nested"
                }))
                .await
                .unwrap();

            assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "nested");
        });
    }

    #[test]
    fn test_write_overwrites_existing() {
        let rt = rt();
        rt.block_on(async {
            let dir = tempfile::tempdir().unwrap();
            let file_path = dir.path().join("overwrite.txt");
            std::fs::write(&file_path, "old content").unwrap();

            WriteFileTool
                .execute(json!({
                    "path": file_path.to_str().unwrap(),
                    "content": "new content"
                }))
                .await
                .unwrap();

            assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "new content");
        });
    }

    #[test]
    fn test_missing_params() {
        let rt = rt();
        rt.block_on(async {
            let r1 = WriteFileTool.execute(json!({ "content": "x" })).await;
            assert!(r1.is_err());

            let r2 = WriteFileTool.execute(json!({ "path": "/tmp/x" })).await;
            assert!(r2.is_err());
        });
    }
}
