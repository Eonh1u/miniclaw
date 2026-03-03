//! Trusted workspace persistence for auto-approving tool confirmations.
//!
//! When a workspace is trusted, dangerous tool calls are auto-approved.
//! Stored in ~/.miniclaw/trusted_workspaces.json.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const TRUSTED_WORKSPACES_FILE: &str = "trusted_workspaces.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrustedWorkspacesData {
    workspaces: Vec<String>,
}

fn trusted_workspaces_path() -> Result<PathBuf> {
    let dir = dirs::home_dir()
        .context("Cannot determine home directory")?
        .join(".miniclaw");
    std::fs::create_dir_all(&dir).context("Failed to create ~/.miniclaw")?;
    Ok(dir.join(TRUSTED_WORKSPACES_FILE))
}

fn load_data() -> Result<TrustedWorkspacesData> {
    let path = trusted_workspaces_path()?;
    if !path.exists() {
        return Ok(TrustedWorkspacesData {
            workspaces: Vec::new(),
        });
    }
    let content =
        std::fs::read_to_string(&path).context("Failed to read trusted_workspaces.json")?;
    let data: TrustedWorkspacesData =
        serde_json::from_str(&content).context("Invalid trusted_workspaces.json")?;
    Ok(data)
}

fn save_data(data: &TrustedWorkspacesData) -> Result<()> {
    let path = trusted_workspaces_path()?;
    let content = serde_json::to_string_pretty(data).context("Failed to serialize")?;
    std::fs::write(&path, content).context("Failed to write trusted_workspaces.json")?;
    Ok(())
}

pub fn is_trusted(workspace: &Path) -> Result<bool> {
    let canonical = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    let canonical_str = canonical.to_string_lossy().to_string();
    let data = load_data()?;
    Ok(data.workspaces.contains(&canonical_str))
}

pub fn add_trusted(workspace: &Path) -> Result<()> {
    let canonical = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    let canonical_str = canonical.to_string_lossy().to_string();
    let mut data = load_data()?;
    if !data.workspaces.contains(&canonical_str) {
        data.workspaces.push(canonical_str);
        save_data(&data)?;
    }
    Ok(())
}

pub fn remove_trusted(workspace: &Path) -> Result<()> {
    let canonical = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    let canonical_str = canonical.to_string_lossy().to_string();
    let mut data = load_data()?;
    data.workspaces.retain(|p| p != &canonical_str);
    save_data(&data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_trusted_and_is_trusted_and_remove() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();

        assert!(!is_trusted(path).unwrap());

        add_trusted(path).unwrap();
        assert!(is_trusted(path).unwrap());

        remove_trusted(path).unwrap();
        assert!(!is_trusted(path).unwrap());
    }
}
