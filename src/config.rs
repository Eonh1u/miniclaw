//! Configuration management for miniclaw.
//!
//! Loads settings from a TOML config file (~/.miniclaw/config.toml)
//! and merges with environment variable overrides.
//!
//! Key concepts:
//! - TOML format: a simple, readable config file format popular in Rust
//! - Environment variables: override config values (especially for secrets like API keys)
//! - Default values: sensible defaults so the tool works out of the box

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Top-level configuration structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub llm: LlmConfig,
    pub agent: AgentConfig,
    pub tools: ToolsConfig,
}

/// LLM provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Which provider to use: "anthropic" or "openai_compatible"
    /// - "anthropic": Anthropic Messages API (Claude)
    /// - "openai_compatible": Any OpenAI-compatible API (OpenAI, Qwen, DeepSeek, etc.)
    pub provider: String,
    /// The model name (e.g. "claude-sonnet-4-20250514", "qwen-plus", "deepseek-chat")
    pub model: String,
    /// Custom API base URL (optional, uses provider default if not set)
    /// Examples:
    ///   - Anthropic:  "https://api.anthropic.com"
    ///   - OpenAI:     "https://api.openai.com/v1"
    ///   - Qwen:       "https://dashscope.aliyuncs.com/compatible-mode/v1"
    ///   - DeepSeek:   "https://api.deepseek.com/v1"
    ///   - Local:      "http://localhost:11434/v1"
    #[serde(default)]
    pub api_base: Option<String>,
    /// Environment variable name that holds the API key
    pub api_key_env: String,
    /// Maximum tokens for LLM responses
    pub max_tokens: u32,
}

/// Agent behavior configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Maximum number of tool-call iterations before stopping
    pub max_iterations: u32,
    /// System prompt that defines the AI's behavior
    pub system_prompt: String,
}

/// Tools configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    /// List of enabled tool names
    pub enabled: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            llm: LlmConfig {
                provider: "openai_compatible".to_string(),
                model: "qwen-plus".to_string(),
                api_base: Some("https://dashscope.aliyuncs.com/compatible-mode/v1".to_string()),
                api_key_env: "LLM_API_KEY".to_string(),
                max_tokens: 4096,
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
        }
    }
}

impl AppConfig {
    /// Get the path to the config file: ~/.miniclaw/config.toml
    pub fn config_path() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        Ok(home.join(".miniclaw").join("config.toml"))
    }

    /// Load config from file, falling back to defaults.
    ///
    /// Priority:
    /// 1. Config file (~/.miniclaw/config.toml) if it exists
    /// 2. Default values
    /// 3. Environment variable overrides (for API key)
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        let mut config = if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;
            toml::from_str(&content)
                .with_context(|| format!("Failed to parse config file: {}", config_path.display()))?
        } else {
            Self::default()
        };

        // Environment variable overrides
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

    /// Resolve the actual API key value from the environment variable.
    pub fn api_key(&self) -> Result<String> {
        std::env::var(&self.llm.api_key_env).with_context(|| {
            format!(
                "API key not found. Please set the {} environment variable.",
                self.llm.api_key_env
            )
        })
    }

    /// Save the default config to ~/.miniclaw/config.toml (for first-time setup).
    pub fn save_default() -> Result<PathBuf> {
        let config_path = Self::config_path()?;
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
        }
        let default = Self::default();
        let content = toml::to_string_pretty(&default).context("Failed to serialize config")?;
        std::fs::write(&config_path, content)
            .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;
        Ok(config_path)
    }
}
