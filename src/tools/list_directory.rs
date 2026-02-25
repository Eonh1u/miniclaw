//! List Directory tool implementation.
//!
//! Lists files and subdirectories within a given path, with optional
//! recursive traversal up to a configurable depth.

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::json;
use std::path::Path;

use super::Tool;

pub struct ListDirectoryTool;

const DEFAULT_MAX_DEPTH: u32 = 3;
const MAX_ENTRIES: usize = 500;

#[async_trait]
impl Tool for ListDirectoryTool {
    fn name(&self) -> &str {
        "list_directory"
    }

    fn description(&self) -> &str {
        "List files and directories at the given path. \
         Supports recursive listing with configurable depth. \
         Returns a tree-style listing with file sizes."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The directory path to list"
                },
                "recursive": {
                    "type": "boolean",
                    "description": "Whether to list recursively (default: false)"
                },
                "max_depth": {
                    "type": "integer",
                    "description": "Maximum recursion depth (default: 3, only used when recursive is true)"
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

        let recursive = params
            .get("recursive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let max_depth = params
            .get("max_depth")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(DEFAULT_MAX_DEPTH);

        let dir_path = Path::new(path);
        if !dir_path.exists() {
            anyhow::bail!("Path does not exist: {}", path);
        }
        if !dir_path.is_dir() {
            anyhow::bail!("Path is not a directory: {}", path);
        }

        let mut entries = Vec::new();
        collect_entries(dir_path, dir_path, recursive, max_depth, 0, &mut entries)?;

        if entries.is_empty() {
            return Ok(format!("{} (empty directory)", path));
        }

        let truncated = entries.len() >= MAX_ENTRIES;
        if truncated {
            entries.truncate(MAX_ENTRIES);
        }

        let mut output = format!("{}  ({} entries)\n", path, entries.len());
        for entry in &entries {
            output.push_str(entry);
            output.push('\n');
        }
        if truncated {
            output.push_str(&format!("... (truncated at {} entries)\n", MAX_ENTRIES));
        }

        Ok(output)
    }
}

fn collect_entries(
    base: &Path,
    dir: &Path,
    recursive: bool,
    max_depth: u32,
    current_depth: u32,
    entries: &mut Vec<String>,
) -> Result<()> {
    let mut dir_entries: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("Failed to read directory: {}", dir.display()))?
        .filter_map(|e| e.ok())
        .collect();

    dir_entries.sort_by_key(|e| e.file_name());

    let indent = "  ".repeat(current_depth as usize);

    for entry in dir_entries {
        if entries.len() >= MAX_ENTRIES {
            return Ok(());
        }

        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();

        // Skip hidden files/dirs at depth 0 to reduce noise
        if current_depth == 0 && name.starts_with('.') {
            continue;
        }

        let metadata = entry.metadata();
        let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);

        if is_dir {
            entries.push(format!("{}📁 {}/", indent, name));
            if recursive && current_depth < max_depth {
                collect_entries(base, &entry.path(), recursive, max_depth, current_depth + 1, entries)?;
            }
        } else {
            let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
            entries.push(format!("{}  {} ({})", indent, name, format_size(size)));
        }
    }

    Ok(())
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
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
        let tool = ListDirectoryTool;
        assert_eq!(tool.name(), "list_directory");
        assert!(!tool.description().is_empty());
        let schema = tool.parameters_schema();
        assert_eq!(schema["required"][0], "path");
    }

    #[test]
    fn test_list_flat_directory() {
        let rt = rt();
        rt.block_on(async {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("a.txt"), "aaa").unwrap();
            std::fs::write(dir.path().join("b.txt"), "bb").unwrap();
            std::fs::create_dir(dir.path().join("subdir")).unwrap();

            let result = ListDirectoryTool
                .execute(json!({ "path": dir.path().to_str().unwrap() }))
                .await
                .unwrap();

            assert!(result.contains("a.txt"));
            assert!(result.contains("b.txt"));
            assert!(result.contains("subdir/"));
        });
    }

    #[test]
    fn test_list_recursive() {
        let rt = rt();
        rt.block_on(async {
            let dir = tempfile::tempdir().unwrap();
            let sub = dir.path().join("child");
            std::fs::create_dir(&sub).unwrap();
            std::fs::write(sub.join("deep.txt"), "deep").unwrap();

            let result = ListDirectoryTool
                .execute(json!({
                    "path": dir.path().to_str().unwrap(),
                    "recursive": true
                }))
                .await
                .unwrap();

            assert!(result.contains("child/"));
            assert!(result.contains("deep.txt"));
        });
    }

    #[test]
    fn test_list_empty_directory() {
        let rt = rt();
        rt.block_on(async {
            let dir = tempfile::tempdir().unwrap();

            let result = ListDirectoryTool
                .execute(json!({ "path": dir.path().to_str().unwrap() }))
                .await
                .unwrap();

            assert!(result.contains("empty directory"));
        });
    }

    #[test]
    fn test_nonexistent_path() {
        let rt = rt();
        rt.block_on(async {
            let result = ListDirectoryTool
                .execute(json!({ "path": "/tmp/__miniclaw_no_such_dir__" }))
                .await;

            assert!(result.is_err());
        });
    }

    #[test]
    fn test_path_is_file_not_dir() {
        let rt = rt();
        rt.block_on(async {
            let tmp = tempfile::NamedTempFile::new().unwrap();

            let result = ListDirectoryTool
                .execute(json!({ "path": tmp.path().to_str().unwrap() }))
                .await;

            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("not a directory"));
        });
    }

    #[test]
    fn test_format_size_units() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(1_048_576), "1.0 MB");
        assert_eq!(format_size(2_621_440), "2.5 MB");
    }
}
