//! Configuration management for miniclaw.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub llm: LlmConfig,
    pub agent: AgentConfig,
    pub tools: ToolsConfig,
    #[serde(default)]
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub api_base: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_api_key_env")]
    pub api_key_env: String,
    pub max_tokens: u32,
    #[serde(default = "default_context_window")]
    pub context_window: u64,
}

fn default_context_window() -> u64 {
    131072 // 128K tokens, common for modern models
}

fn default_api_key_env() -> String {
    "LLM_API_KEY".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub max_iterations: u32,
    pub system_prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    pub enabled: Vec<String>,
}

/// UI widget visibility configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Show the stats panel in the header (token counts, usage days).
    #[serde(default = "bool_true")]
    pub show_stats: bool,
    /// Show the pet animation panel in the header.
    #[serde(default = "bool_true")]
    pub show_pet: bool,
}

fn bool_true() -> bool {
    true
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            show_stats: true,
            show_pet: true,
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            llm: LlmConfig {
                provider: "openai_compatible".to_string(),
                model: "qwen-plus".to_string(),
                api_base: Some("https://dashscope.aliyuncs.com/compatible-mode/v1".to_string()),
                api_key: None,
                api_key_env: "LLM_API_KEY".to_string(),
                max_tokens: 4096,
                context_window: default_context_window(),
            },
            agent: AgentConfig {
                max_iterations: 20,
                system_prompt: "You are a helpful AI assistant. You can use tools to help \
                    the user with tasks like reading files, writing files, executing commands, \
                    and more. Be concise and helpful."
                    .to_string(),
            },
            tools: ToolsConfig {
                enabled: vec![
                    "read_file".to_string(),
                    "write_file".to_string(),
                    "list_directory".to_string(),
                    "exec_command".to_string(),
                ],
            },
            ui: UiConfig::default(),
        }
    }
}

impl AppConfig {
    pub fn config_path() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        Ok(home.join(".miniclaw").join("config.toml"))
    }

    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        let mut config = if config_path.exists() {
            let content = std::fs::read_to_string(&config_path).with_context(|| {
                format!("Failed to read config file: {}", config_path.display())
            })?;
            toml::from_str(&content).with_context(|| {
                format!("Failed to parse config file: {}", config_path.display())
            })?
        } else {
            Self::default()
        };

        if let Ok(provider) = std::env::var("MINICLAW_PROVIDER") {
            config.llm.provider = provider;
        }
        if let Ok(model) = std::env::var("MINICLAW_MODEL") {
            config.llm.model = model;
        }
        if let Ok(api_base) = std::env::var("MINICLAW_API_BASE") {
            config.llm.api_base = Some(api_base);
        }

        Ok(config)
    }

    pub fn api_key(&self) -> Result<String> {
        if let Some(key) = &self.llm.api_key {
            if !key.is_empty() {
                return Ok(key.clone());
            }
        }
        std::env::var(&self.llm.api_key_env).with_context(|| {
            format!(
                "API key not found. Either:\n  \
                 1. Set api_key in config file: {}\n  \
                 2. Set environment variable: export {}=your-key",
                Self::config_path()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default(),
                self.llm.api_key_env
            )
        })
    }

    pub fn save_default() -> Result<PathBuf> {
        let config_path = Self::config_path()?;
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory: {}", parent.display())
            })?;
        }
        let default = Self::default();
        let content = toml::to_string_pretty(&default).context("Failed to serialize config")?;
        std::fs::write(&config_path, content)
            .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;
        Ok(config_path)
    }
}
