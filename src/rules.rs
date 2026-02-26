//! Rule file discovery and loading.
//!
//! Mimics Claude Code's CLAUDE.md resolution strategy:
//! 1. Walk upward from the project root, collecting CLAUDE.md files.
//! 2. Include CLAUDE.md in the project root and .claude/ subdirectory.
//!
//! Discovered content is concatenated (ancestors first, then project root)
//! and returned as a string for injection into the system prompt.

use std::path::{Path, PathBuf};

/// A single rule file discovered on disk.
#[derive(Debug, Clone)]
pub struct RuleFile {
    pub path: PathBuf,
    pub content: String,
}

/// Discover and load all CLAUDE.md rule files relative to `project_root`.
///
/// Search order (earliest ancestor first, project root last):
/// 1. Ancestor directories (filesystem root down to parent of project root)
/// 2. `<project_root>/CLAUDE.md`
/// 3. `<project_root>/.claude/CLAUDE.md`
pub fn load_rules(project_root: &Path) -> Vec<RuleFile> {
    let project_root = match project_root.canonicalize() {
        Ok(p) => p,
        Err(_) => project_root.to_path_buf(),
    };

    let mut ancestor_rules = collect_ancestor_rules(&project_root);
    ancestor_rules.reverse(); // filesystem root first

    let mut rules: Vec<RuleFile> = ancestor_rules;

    try_load(&project_root.join("CLAUDE.md"), &mut rules);
    try_load(&project_root.join(".claude").join("CLAUDE.md"), &mut rules);

    rules
}

/// Build a combined rules string ready for system prompt injection.
/// Returns `None` if no rule files were found.
pub fn build_rules_context(project_root: &Path) -> Option<String> {
    let rules = load_rules(project_root);
    if rules.is_empty() {
        return None;
    }

    let mut parts: Vec<String> = Vec::with_capacity(rules.len());
    for rule in &rules {
        let header = format!("# Rules from {}", rule.path.display());
        parts.push(format!("{}\n\n{}", header, rule.content.trim()));
    }

    Some(parts.join("\n\n---\n\n"))
}

fn collect_ancestor_rules(project_root: &Path) -> Vec<RuleFile> {
    let mut results = Vec::new();
    let mut current = project_root.parent();
    while let Some(dir) = current {
        try_load(&dir.join("CLAUDE.md"), &mut results);
        try_load(&dir.join(".claude").join("CLAUDE.md"), &mut results);
        current = dir.parent();
    }
    results
}

fn try_load(path: &Path, out: &mut Vec<RuleFile>) {
    if path.is_file() {
        if let Ok(content) = std::fs::read_to_string(path) {
            if !content.trim().is_empty() {
                out.push(RuleFile {
                    path: path.to_path_buf(),
                    content,
                });
            }
        }
    }
}
