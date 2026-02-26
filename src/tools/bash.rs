//! Bash tool implementation.
//!
//! Executes shell commands via `bash -c`, with timeout control
//! and output truncation for safety.

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::json;

use super::Tool;

pub struct BashTool;

const DEFAULT_TIMEOUT_SECS: u64 = 30;
const MAX_OUTPUT_BYTES: usize = 100_000;

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute a shell command via bash. Returns stdout and stderr. \
         Use this for running build commands, searching files (grep/rg/find), \
         git operations, listing directories, installing packages, etc. \
         Commands run with a configurable timeout (default 30s)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds (default: 30, max: 300)"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> Result<String> {
        let command = params
            .get("command")
            .and_then(|v| v.as_str())
            .context("Missing required parameter: command")?;

        let timeout_secs = params
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_TIMEOUT_SECS)
            .min(300);

        let cmd_clone = command.to_string();
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            tokio::process::Command::new("bash")
                .arg("-c")
                .arg(&cmd_clone)
                .output(),
        )
        .await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let exit_code = output.status.code().unwrap_or(-1);

                let mut result = String::new();

                if !stdout.is_empty() {
                    let truncated = truncate_output(&stdout, MAX_OUTPUT_BYTES);
                    result.push_str(&truncated);
                }
                if !stderr.is_empty() {
                    if !result.is_empty() {
                        result.push('\n');
                    }
                    result.push_str("[stderr]\n");
                    let truncated = truncate_output(&stderr, MAX_OUTPUT_BYTES / 2);
                    result.push_str(&truncated);
                }

                if result.is_empty() {
                    result = format!("(no output, exit code: {})", exit_code);
                } else if exit_code != 0 {
                    result.push_str(&format!("\n[exit code: {}]", exit_code));
                }

                Ok(result)
            }
            Ok(Err(e)) => Err(anyhow::anyhow!("Failed to execute command: {}", e)),
            Err(_) => Err(anyhow::anyhow!(
                "Command timed out after {}s: {}",
                timeout_secs,
                command
            )),
        }
    }
}

fn truncate_output(output: &str, max_bytes: usize) -> String {
    if output.len() <= max_bytes {
        return output.to_string();
    }
    let half = max_bytes / 2;
    let start = &output[..output.floor_char_boundary(half)];
    let end = &output[output.ceil_char_boundary(output.len() - half)..];
    let omitted = output.len() - start.len() - end.len();
    format!(
        "{}\n\n... ({} bytes omitted) ...\n\n{}",
        start, omitted, end
    )
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
        let tool = BashTool;
        assert_eq!(tool.name(), "bash");
        assert!(!tool.description().is_empty());
        let schema = tool.parameters_schema();
        assert_eq!(schema["required"][0], "command");
    }

    #[test]
    fn test_echo_command() {
        let rt = rt();
        rt.block_on(async {
            let result = BashTool
                .execute(json!({ "command": "echo hello" }))
                .await
                .unwrap();
            assert_eq!(result.trim(), "hello");
        });
    }

    #[test]
    fn test_exit_code() {
        let rt = rt();
        rt.block_on(async {
            let result = BashTool
                .execute(json!({ "command": "exit 42" }))
                .await
                .unwrap();
            assert!(result.contains("exit code: 42"));
        });
    }

    #[test]
    fn test_stderr_capture() {
        let rt = rt();
        rt.block_on(async {
            let result = BashTool
                .execute(json!({ "command": "echo error >&2" }))
                .await
                .unwrap();
            assert!(result.contains("[stderr]"));
            assert!(result.contains("error"));
        });
    }

    #[test]
    fn test_timeout() {
        let rt = rt();
        rt.block_on(async {
            let result = BashTool
                .execute(json!({ "command": "sleep 10", "timeout": 1 }))
                .await;
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("timed out"));
        });
    }

    #[test]
    fn test_missing_command() {
        let rt = rt();
        rt.block_on(async {
            let result = BashTool.execute(json!({})).await;
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("command"));
        });
    }

    #[test]
    fn test_multiline_output() {
        let rt = rt();
        rt.block_on(async {
            let result = BashTool
                .execute(json!({ "command": "echo line1; echo line2; echo line3" }))
                .await
                .unwrap();
            assert!(result.contains("line1"));
            assert!(result.contains("line2"));
            assert!(result.contains("line3"));
        });
    }

    #[test]
    fn test_truncate_output() {
        let long = "a".repeat(200);
        let truncated = truncate_output(&long, 100);
        assert!(truncated.contains("omitted"));
        assert!(truncated.len() < 200);
    }
}
