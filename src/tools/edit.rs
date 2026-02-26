//! Edit tool implementation.
//!
//! Performs precise text replacement in files by matching an exact
//! old_text string and replacing it with new_text. This is safer than
//! overwriting the entire file, as it requires the caller to prove
//! they know the current content.

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde_json::json;

use super::Tool;

pub struct EditTool;

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }

    fn description(&self) -> &str {
        "Make a precise text replacement in a file. You must provide the exact text \
         to find (old_text) and the replacement text (new_text). The old_text must \
         match exactly (including whitespace and indentation). Only the first \
         occurrence is replaced by default; set replace_all to true to replace all."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to edit"
                },
                "old_text": {
                    "type": "string",
                    "description": "The exact text to find in the file (must match precisely)"
                },
                "new_text": {
                    "type": "string",
                    "description": "The text to replace old_text with"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "If true, replace all occurrences (default: false)"
                }
            },
            "required": ["path", "old_text", "new_text"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> Result<String> {
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .context("Missing required parameter: path")?;

        let old_text = params
            .get("old_text")
            .and_then(|v| v.as_str())
            .context("Missing required parameter: old_text")?;

        let new_text = params
            .get("new_text")
            .and_then(|v| v.as_str())
            .context("Missing required parameter: new_text")?;

        let replace_all = params
            .get("replace_all")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let content = tokio::fs::read_to_string(path)
            .await
            .with_context(|| format!("Failed to read file: {}", path))?;

        if !content.contains(old_text) {
            let preview = if old_text.len() > 80 {
                format!("{}...", &old_text[..old_text.floor_char_boundary(80)])
            } else {
                old_text.to_string()
            };
            bail!(
                "old_text not found in {}. Make sure it matches exactly \
                 (including whitespace and indentation).\nSearched for: {:?}",
                path,
                preview
            );
        }

        let (new_content, count) = if replace_all {
            let count = content.matches(old_text).count();
            (content.replace(old_text, new_text), count)
        } else {
            (content.replacen(old_text, new_text, 1), 1)
        };

        tokio::fs::write(path, &new_content)
            .await
            .with_context(|| format!("Failed to write file: {}", path))?;

        Ok(format!(
            "Successfully replaced {} occurrence(s) in {}",
            count, path
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
        let tool = EditTool;
        assert_eq!(tool.name(), "edit");
        assert!(!tool.description().is_empty());
        let schema = tool.parameters_schema();
        let required = schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "path"));
        assert!(required.iter().any(|v| v == "old_text"));
        assert!(required.iter().any(|v| v == "new_text"));
    }

    #[test]
    fn test_replace_single() {
        let rt = rt();
        rt.block_on(async {
            let dir = tempfile::tempdir().unwrap();
            let file = dir.path().join("test.txt");
            std::fs::write(&file, "hello world hello").unwrap();

            let result = EditTool
                .execute(json!({
                    "path": file.to_str().unwrap(),
                    "old_text": "hello",
                    "new_text": "hi"
                }))
                .await
                .unwrap();

            assert!(result.contains("1 occurrence"));
            let content = std::fs::read_to_string(&file).unwrap();
            assert_eq!(content, "hi world hello");
        });
    }

    #[test]
    fn test_replace_all() {
        let rt = rt();
        rt.block_on(async {
            let dir = tempfile::tempdir().unwrap();
            let file = dir.path().join("test.txt");
            std::fs::write(&file, "aaa bbb aaa ccc aaa").unwrap();

            let result = EditTool
                .execute(json!({
                    "path": file.to_str().unwrap(),
                    "old_text": "aaa",
                    "new_text": "xxx",
                    "replace_all": true
                }))
                .await
                .unwrap();

            assert!(result.contains("3 occurrence"));
            let content = std::fs::read_to_string(&file).unwrap();
            assert_eq!(content, "xxx bbb xxx ccc xxx");
        });
    }

    #[test]
    fn test_old_text_not_found() {
        let rt = rt();
        rt.block_on(async {
            let dir = tempfile::tempdir().unwrap();
            let file = dir.path().join("test.txt");
            std::fs::write(&file, "hello world").unwrap();

            let result = EditTool
                .execute(json!({
                    "path": file.to_str().unwrap(),
                    "old_text": "xyz",
                    "new_text": "abc"
                }))
                .await;

            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("not found"));
        });
    }

    #[test]
    fn test_preserves_whitespace() {
        let rt = rt();
        rt.block_on(async {
            let dir = tempfile::tempdir().unwrap();
            let file = dir.path().join("test.rs");
            std::fs::write(&file, "fn main() {\n    println!(\"old\");\n}\n").unwrap();

            EditTool
                .execute(json!({
                    "path": file.to_str().unwrap(),
                    "old_text": "    println!(\"old\");",
                    "new_text": "    println!(\"new\");"
                }))
                .await
                .unwrap();

            let content = std::fs::read_to_string(&file).unwrap();
            assert!(content.contains("println!(\"new\")"));
            assert!(content.contains("fn main()"));
        });
    }

    #[test]
    fn test_missing_params() {
        let rt = rt();
        rt.block_on(async {
            let r = EditTool
                .execute(json!({ "path": "/tmp/x", "old_text": "a" }))
                .await;
            assert!(r.is_err());

            let r = EditTool.execute(json!({ "path": "/tmp/x" })).await;
            assert!(r.is_err());
        });
    }

    #[test]
    fn test_nonexistent_file() {
        let rt = rt();
        rt.block_on(async {
            let result = EditTool
                .execute(json!({
                    "path": "/tmp/__miniclaw_no_such_file__",
                    "old_text": "a",
                    "new_text": "b"
                }))
                .await;
            assert!(result.is_err());
        });
    }
}
