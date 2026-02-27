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

/// A single model entry in the models list. Used for multi-model config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    /// Unique id for switching (e.g. "qwen-plus", "deepseek").
    pub id: String,
    /// Display name shown in UI.
    #[serde(default)]
    pub name: String,
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub api_base: Option<String>,
    /// Context window size in tokens. 0 = use [llm] default.
    #[serde(default)]
    pub context_window: u64,
    /// Max output tokens per response. 0 = use [llm] default.
    #[serde(default)]
    pub max_tokens: u32,
    /// Allowed tool names for this model. Empty = all tools. E.g. ["read_file","write_file","bash"].
    #[serde(default)]
    pub tools: Vec<String>,
    /// Enable web search (e.g. qwen3.5-plus 联网搜索). DashScope/百炼 API: extra_body.enable_search.
    #[serde(default)]
    pub enable_search: bool,
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
    /// Multi-model list. If present, user can switch between models in session.
    #[serde(default)]
    pub models: Vec<ModelEntry>,
    /// Default model id when using models list. Ignored if models is empty.
    #[serde(default)]
    pub default_model: Option<String>,
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
                models: vec![],
                default_model: None,
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

    /// Returns the list of available models. If `models` is empty, derives one from legacy provider/model/api_base.
    pub fn list_models(&self) -> Vec<ModelEntry> {
        if self.llm.models.is_empty() {
            let name = if self.llm.model.is_empty() {
                "default".to_string()
            } else {
                self.llm.model.clone()
            };
            vec![ModelEntry {
                id: self.llm.model.clone(),
                name: name.clone(),
                provider: self.llm.provider.clone(),
                model: self.llm.model.clone(),
                api_base: self.llm.api_base.clone(),
                context_window: self.llm.context_window,
                max_tokens: self.llm.max_tokens,
                tools: vec![],
                enable_search: false,
            }]
        } else {
            self.llm.models.clone()
        }
    }

    /// Returns the default model id for new sessions.
    pub fn default_model_id(&self) -> String {
        let models = self.list_models();
        if models.is_empty() {
            return "default".to_string();
        }
        if let Some(ref default) = self.llm.default_model {
            if models.iter().any(|m| m.id == *default) {
                return default.clone();
            }
        }
        models[0].id.clone()
    }

    /// Get model entry by id. Returns None if not found.
    /// Resolves context_window/max_tokens 0 to [llm] defaults.
    pub fn get_model_entry(&self, id: &str) -> Option<ModelEntry> {
        self.list_models()
            .into_iter()
            .find(|m| m.id == id)
            .map(|mut m| {
                if m.context_window == 0 {
                    m.context_window = self.llm.context_window;
                }
                if m.max_tokens == 0 {
                    m.max_tokens = self.llm.max_tokens;
                }
                m
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_entry_tools_and_enable_search() {
        let toml = r#"
[llm]
provider = "openai_compatible"
model = "qwen-plus"
api_key_env = "LLM_API_KEY"
max_tokens = 4096

[[llm.models]]
id = "qwen3.5-plus"
name = "Qwen 3.5 Plus"
provider = "openai_compatible"
model = "qwen3.5-plus"
api_base = "https://dashscope.aliyuncs.com/compatible-mode/v1"
context_window = 1048576
max_tokens = 8192
tools = ["read_file", "write_file", "bash"]
enable_search = true

[[llm.models]]
id = "qwen-plus"
name = "Qwen Plus"
provider = "openai_compatible"
model = "qwen-plus"
tools = []
enable_search = false

[agent]
max_iterations = 20
system_prompt = "You are a helpful assistant."

[tools]
enabled = ["read_file", "write_file", "list_directory", "exec_command"]
"#;
        let config: AppConfig = toml::from_str(toml).unwrap();
        let models = config.list_models();
        assert_eq!(models.len(), 2);

        let qwen35 = models.iter().find(|m| m.id == "qwen3.5-plus").unwrap();
        assert_eq!(qwen35.tools, ["read_file", "write_file", "bash"]);
        assert!(qwen35.enable_search);

        let qwen = models.iter().find(|m| m.id == "qwen-plus").unwrap();
        assert!(qwen.tools.is_empty());
        assert!(!qwen.enable_search);
    }
}
