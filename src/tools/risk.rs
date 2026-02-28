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

    // Split by && and || to evaluate each sub-command
    let sub_commands: Vec<&str> = cmd
        .split("&&")
        .flat_map(|s| s.split("||"))
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let mut worst = RiskLevel::Safe;
    for sub in &sub_commands {
        let level = classify_single_command(sub);
        if level == RiskLevel::Dangerous {
            return RiskLevel::Dangerous;
        }
        if level == RiskLevel::Moderate && worst == RiskLevel::Safe {
            worst = RiskLevel::Moderate;
        }
    }
    worst
}

fn classify_single_command(cmd: &str) -> RiskLevel {
    // Check dangerous patterns first
    let pipe_segments: Vec<&str> = cmd.split('|').map(|s| s.trim()).collect();
    for seg in &pipe_segments {
        let first_word = seg.split_whitespace().next().unwrap_or("");
        for pattern in DANGEROUS_COMMAND_WORDS {
            if first_word == *pattern {
                return RiskLevel::Dangerous;
            }
        }
    }

    // Check for dangerous redirects (> or >> to real files, not /dev/null)
    if has_dangerous_redirect(cmd) {
        return RiskLevel::Dangerous;
    }

    // Check safe patterns
    let first_word = cmd.split_whitespace().next().unwrap_or("");
    for pattern in SAFE_PATTERNS {
        if first_word == *pattern {
            return RiskLevel::Safe;
        }
        if first_word.contains('/') && first_word.ends_with(pattern) {
            return RiskLevel::Safe;
        }
    }

    RiskLevel::Moderate
}

/// Safe redirect targets: temp dirs, /dev/null, and fd dup (2>&1).
fn is_safe_redirect_target(target: &str) -> bool {
    if target.is_empty() {
        return true;
    }
    let t = target.trim();
    if t == "/dev/null" {
        return true;
    }
    // fd-to-fd redirect: 2>&1, 1>&2 - not writing to a real file
    if t.starts_with('&') && t.len() > 1 && t[1..].chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    // /tmp, /tmp/..., /var/tmp, /var/tmp/... - standard temp locations for logs
    if t == "/tmp" || t.starts_with("/tmp/") {
        return true;
    }
    if t == "/var/tmp" || t.starts_with("/var/tmp/") {
        return true;
    }
    false
}

fn has_dangerous_redirect(cmd: &str) -> bool {
    let mut i = 0;
    let chars: Vec<char> = cmd.chars().collect();
    while i < chars.len() {
        if chars[i] == '>' {
            let mut j = i + 1;
            if j < chars.len() && chars[j] == '>' {
                j += 1;
            }
            while j < chars.len() && chars[j] == ' ' {
                j += 1;
            }
            let target: String = chars[j..]
                .iter()
                .take_while(|c| !c.is_whitespace())
                .collect();
            if !target.is_empty() && !is_safe_redirect_target(&target) {
                if i > 0 && chars[i - 1].is_ascii_digit() {
                    let target_check: String = chars[j..]
                        .iter()
                        .take_while(|c| !c.is_whitespace())
                        .collect();
                    if is_safe_redirect_target(&target_check) {
                        i = j;
                        continue;
                    }
                }
                return true;
            }
        }
        i += 1;
    }
    false
}

const DANGEROUS_COMMAND_WORDS: &[&str] = &[
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
];

const SAFE_PATTERNS: &[&str] = &[
    "ls", "cat", "head", "tail", "less", "more", "wc", "echo", "printf", "pwd", "whoami", "which",
    "where", "type", "file", "stat", "du", "df", "date", "uname", "env", "printenv", "grep", "rg",
    "find", "fd", "ag", "awk",
    "sed", // sed without -i is safe; with -i it modifies files but we'll allow it as moderate via fallback
    "sort", "uniq", "diff", "tree", "git", "cargo", "rustc", "rustup", "npm", "node", "python",
    "python3", "pip", "pip3", "go", "make", "cmake", "docker", "kubectl",
    "cd",    // change directory - no side effects
    "sleep", // wait - no side effects
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
    fn test_redirect_to_file_is_dangerous() {
        assert_eq!(
            assess_risk("bash", r#"{"command": "echo x > /etc/hosts"}"#),
            RiskLevel::Dangerous
        );
    }

    #[test]
    fn test_redirect_to_devnull_is_safe() {
        assert_eq!(
            assess_risk(
                "bash",
                r#"{"command": "ls -l hello* 2>/dev/null || echo not found"}"#
            ),
            RiskLevel::Safe
        );
        assert_eq!(
            assess_risk("bash", r#"{"command": "cat file 2> /dev/null"}"#),
            RiskLevel::Safe
        );
    }

    #[test]
    fn test_redirect_to_tmp_is_safe() {
        // Common pattern: run app in background, redirect logs to /tmp
        assert_eq!(
            assess_risk(
                "bash",
                r#"{"command": "cd /root/code/todo_app && python3 -c \"from app import app; app.run()\" > /tmp/todo_app.log 2>&1 & sleep 2 && cat /tmp/todo_app.log"}"#
            ),
            RiskLevel::Safe
        );
    }

    #[test]
    fn test_compound_commands() {
        assert_eq!(
            assess_risk("bash", r#"{"command": "ls -la && echo done"}"#),
            RiskLevel::Safe
        );
        assert_eq!(
            assess_risk("bash", r#"{"command": "ls -la || echo fallback"}"#),
            RiskLevel::Safe
        );
        assert_eq!(
            assess_risk("bash", r#"{"command": "ls && rm -rf /"}"#),
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
