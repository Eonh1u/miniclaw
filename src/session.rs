//! Session persistence and multi-session management.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::agent::SessionStats;
use crate::types::Message;

/// Persistent session data saved to disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub agent_messages: Vec<Message>,
    pub ui_messages: Vec<String>,
    pub stats: SessionStatsData,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionStatsData {
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub request_count: u64,
}

impl From<&SessionStats> for SessionStatsData {
    fn from(stats: &SessionStats) -> Self {
        Self {
            total_input_tokens: stats.total_input_tokens,
            total_output_tokens: stats.total_output_tokens,
            request_count: stats.request_count,
        }
    }
}

impl SessionStatsData {
    pub fn to_session_stats(&self) -> SessionStats {
        SessionStats {
            total_input_tokens: self.total_input_tokens,
            total_output_tokens: self.total_output_tokens,
            request_count: self.request_count,
        }
    }
}

fn sessions_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    let dir = home.join(".miniclaw").join("sessions");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn save_session(data: &SessionData) -> Result<PathBuf> {
    let dir = sessions_dir()?;
    let path = dir.join(format!("{}.json", data.id));
    let json = serde_json::to_string_pretty(data)?;
    std::fs::write(&path, &json)?;
    Ok(path)
}

pub fn load_session(id: &str) -> Result<SessionData> {
    let dir = sessions_dir()?;
    let path = dir.join(format!("{}.json", id));
    let content =
        std::fs::read_to_string(&path).with_context(|| format!("Session '{}' not found", id))?;
    let data: SessionData = serde_json::from_str(&content)?;
    Ok(data)
}

pub fn list_sessions() -> Result<Vec<SessionData>> {
    let dir = sessions_dir()?;
    let mut sessions = Vec::new();
    if !dir.exists() {
        return Ok(sessions);
    }
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "json") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(data) = serde_json::from_str::<SessionData>(&content) {
                    sessions.push(data);
                }
            }
        }
    }
    sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(sessions)
}

pub fn export_session(data: &SessionData, path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(data)?;
    std::fs::write(path, json)?;
    Ok(())
}

pub fn import_session(path: &Path) -> Result<SessionData> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("Cannot read {}", path.display()))?;
    let data: SessionData = serde_json::from_str(&content)?;
    Ok(data)
}

pub fn generate_session_id() -> String {
    uuid::Uuid::new_v4().to_string()[..8].to_string()
}

pub fn now_timestamp() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_session_id() {
        let id = generate_session_id();
        assert_eq!(id.len(), 8);
    }

    #[test]
    fn test_session_data_roundtrip() {
        let data = SessionData {
            id: "test123".to_string(),
            name: "Test Session".to_string(),
            created_at: now_timestamp(),
            agent_messages: vec![],
            ui_messages: vec!["Hello".to_string()],
            stats: SessionStatsData::default(),
        };
        let json = serde_json::to_string(&data).unwrap();
        let loaded: SessionData = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.id, "test123");
        assert_eq!(loaded.name, "Test Session");
        assert_eq!(loaded.ui_messages.len(), 1);
    }

    #[test]
    fn test_stats_conversion() {
        let stats = SessionStats {
            total_input_tokens: 100,
            total_output_tokens: 50,
            request_count: 3,
        };
        let data = SessionStatsData::from(&stats);
        assert_eq!(data.total_input_tokens, 100);
        let back = data.to_session_stats();
        assert_eq!(back.total_output_tokens, 50);
        assert_eq!(back.request_count, 3);
    }

    #[test]
    fn test_export_import() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_session.json");
        let data = SessionData {
            id: "exp1".to_string(),
            name: "Export Test".to_string(),
            created_at: now_timestamp(),
            agent_messages: vec![],
            ui_messages: vec!["msg1".to_string(), "msg2".to_string()],
            stats: SessionStatsData::default(),
        };
        export_session(&data, &path).unwrap();
        let loaded = import_session(&path).unwrap();
        assert_eq!(loaded.id, "exp1");
        assert_eq!(loaded.ui_messages.len(), 2);
    }
}
