//! Persist Telegram chat state (e.g. model per chat) to ~/.miniclaw/telegram_state.json

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::RwLock;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TelegramState {
    /// chat_id (as string) -> model_id
    #[serde(default)]
    pub chat_models: HashMap<String, String>,
}

impl TelegramState {
    fn path() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        Ok(home.join(".miniclaw").join("telegram_state.json"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))
    }

    fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
        let content =
            serde_json::to_string_pretty(self).context("Failed to serialize telegram state")?;
        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write {}", path.display()))
    }

    pub fn get_model(&self, chat_id: i64) -> Option<String> {
        self.chat_models.get(&chat_id.to_string()).cloned()
    }

    pub fn set_model(&mut self, chat_id: i64, model_id: String) -> Result<()> {
        self.chat_models.insert(chat_id.to_string(), model_id);
        self.save()
    }
}

/// Shared state with async-safe access
pub struct TelegramStateStore {
    inner: RwLock<TelegramState>,
}

impl TelegramStateStore {
    pub fn new() -> Result<Self> {
        let state = TelegramState::load()?;
        Ok(Self {
            inner: RwLock::new(state),
        })
    }

    pub async fn get_model(&self, chat_id: i64) -> Option<String> {
        self.inner.read().await.get_model(chat_id)
    }

    pub async fn set_model(&self, chat_id: i64, model_id: String) -> Result<()> {
        self.inner.write().await.set_model(chat_id, model_id)
    }
}
