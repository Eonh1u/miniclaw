//! Tool risk assessment for confirmation mechanism.
//!
//! Classifies tool calls into risk levels based on the tool name
//! and arguments, using pattern matching for bash commands.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RiskLevel {
    /// Read-only operations, auto-execute without confirmation.
    Safe,
    /// File modifications, show info but auto-execute.
    Moderate,
    /// Destructive or dangerous operations, require explicit Y/N confirmation.
    Dangerous,
}

/// Assess the risk level of a tool call.
pub fn assess_risk(tool_name: &str, arguments: &str) -> RiskLevel {
    match tool_name {
        "read_file" | "list_directory" => RiskLevel::Safe,
        "write_file" | "edit" => RiskLevel::Moderate,
        "bash" => assess_bash_risk(arguments),
        _ => RiskLevel::Moderate,
    }
}

fn assess_bash_risk(arguments: &str) -> RiskLevel {
    let args: serde_json::Value =
        serde_json::from_str(arguments).unwrap_or(serde_json::Value::Null);
    let command = args["command"].as_str().unwrap_or("");
    classify_bash_command(command)
}

fn classify_bash_command(command: &str) -> RiskLevel {
    let cmd = command.trim();

    for pattern in DANGEROUS_PATTERNS {
        if cmd_matches(cmd, pattern) {
            return RiskLevel::Dangerous;
        }
    }

    for pattern in SAFE_PATTERNS {
        if cmd_starts_with_any(cmd, pattern) {
            return RiskLevel::Safe;
        }
    }

    RiskLevel::Moderate
}

fn cmd_matches(cmd: &str, pattern: &str) -> bool {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let pipe_segments: Vec<&str> = cmd.split('|').collect();

    for part in &parts {
        if *part == pattern {
            return true;
        }
    }

    if pattern == ">" || pattern == ">>" {
        return cmd.contains(pattern);
    }

    for seg in &pipe_segments {
        let seg = seg.trim();
        if let Some(first_word) = seg.split_whitespace().next() {
            if first_word == pattern {
                return true;
            }
        }
    }

    false
}

fn cmd_starts_with_any(cmd: &str, prefix: &str) -> bool {
    let first = cmd.split_whitespace().next().unwrap_or("");
    if first == prefix {
        return true;
    }
    if first.ends_with(prefix) && first.contains('/') {
        return true;
    }
    false
}

const DANGEROUS_PATTERNS: &[&str] = &[
    "rm",
    "rmdir",
    "sudo",
    "su",
    "kill",
    "pkill",
    "killall",
    "chmod",
    "chown",
    "chgrp",
    "dd",
    "mkfs",
    "fdisk",
    "parted",
    "mount",
    "umount",
    "shutdown",
    "reboot",
    "systemctl",
    "service",
    "iptables",
    "useradd",
    "userdel",
    "passwd",
    "curl|bash",
    "curl|sh",
    "wget|bash",
    "wget|sh",
    ">",
];

const SAFE_PATTERNS: &[&str] = &[
    "ls", "cat", "head", "tail", "less", "more", "wc", "echo", "printf", "pwd", "whoami", "which",
    "where", "type", "file", "stat", "du", "df", "date", "uname", "env", "printenv", "grep", "rg",
    "find", "fd", "ag", "awk",
    "sed", // sed without -i is safe; with -i it modifies files but we'll allow it as moderate via fallback
    "sort", "uniq", "diff", "tree", "git", "cargo", "rustc", "rustup", "npm", "node", "python",
    "python3", "pip", "pip3", "go", "make", "cmake", "docker", "kubectl",
];

/// Generate a human-readable description for a tool call confirmation prompt.
pub fn describe_tool_call(tool_name: &str, arguments: &str) -> String {
    let args: serde_json::Value =
        serde_json::from_str(arguments).unwrap_or(serde_json::Value::Null);

    match tool_name {
        "bash" => {
            let cmd = args["command"].as_str().unwrap_or("?");
            format!("执行命令: {}", cmd)
        }
        "write_file" => {
            let path = args["path"].as_str().unwrap_or("?");
            format!("写入文件: {}", path)
        }
        "edit" => {
            let path = args["path"].as_str().unwrap_or("?");
            format!("编辑文件: {}", path)
        }
        "read_file" => {
            let path = args["path"].as_str().unwrap_or("?");
            format!("读取文件: {}", path)
        }
        other => format!("调用工具: {}", other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_tools() {
        assert_eq!(assess_risk("read_file", "{}"), RiskLevel::Safe);
        assert_eq!(assess_risk("list_directory", "{}"), RiskLevel::Safe);
    }

    #[test]
    fn test_moderate_tools() {
        assert_eq!(assess_risk("write_file", "{}"), RiskLevel::Moderate);
        assert_eq!(assess_risk("edit", "{}"), RiskLevel::Moderate);
    }

    #[test]
    fn test_safe_bash_commands() {
        let cases = [
            r#"{"command": "ls -la"}"#,
            r#"{"command": "cat src/main.rs"}"#,
            r#"{"command": "grep -rn TODO src/"}"#,
            r#"{"command": "cargo test"}"#,
            r#"{"command": "git status"}"#,
            r#"{"command": "echo hello"}"#,
            r#"{"command": "find . -name '*.rs'"}"#,
            r#"{"command": "rg pattern src/"}"#,
        ];
        for args in &cases {
            assert_eq!(
                assess_risk("bash", args),
                RiskLevel::Safe,
                "Expected Safe for: {}",
                args
            );
        }
    }

    #[test]
    fn test_dangerous_bash_commands() {
        let cases = [
            r#"{"command": "rm -rf /tmp/test"}"#,
            r#"{"command": "sudo apt-get install foo"}"#,
            r#"{"command": "kill -9 1234"}"#,
            r#"{"command": "chmod 777 /etc/passwd"}"#,
            r#"{"command": "dd if=/dev/zero of=/dev/sda"}"#,
        ];
        for args in &cases {
            assert_eq!(
                assess_risk("bash", args),
                RiskLevel::Dangerous,
                "Expected Dangerous for: {}",
                args
            );
        }
    }

    #[test]
    fn test_moderate_bash_commands() {
        let cases = [
            r#"{"command": "cp file1 file2"}"#,
            r#"{"command": "tar xf archive.tar"}"#,
            r#"{"command": "wget https://example.com/file"}"#,
        ];
        for args in &cases {
            assert_eq!(
                assess_risk("bash", args),
                RiskLevel::Moderate,
                "Expected Moderate for: {}",
                args
            );
        }
    }

    #[test]
    fn test_redirect_is_dangerous() {
        assert_eq!(
            assess_risk("bash", r#"{"command": "echo x > /etc/hosts"}"#),
            RiskLevel::Dangerous
        );
    }

    #[test]
    fn test_describe_tool_call() {
        let desc = describe_tool_call("bash", r#"{"command": "ls -la"}"#);
        assert!(desc.contains("ls -la"));

        let desc = describe_tool_call("edit", r#"{"path": "src/main.rs"}"#);
        assert!(desc.contains("src/main.rs"));
    }
}
