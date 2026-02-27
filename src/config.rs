//! Configuration management for miniclaw.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub llm: LlmConfig,
    pub agent: AgentConfig,
    pub tools: ToolsConfig,
    #[serde(default)]
    pub ui: UiConfig,
}

/// Provider config: unified api_base, api_key, and api format. Models under a provider inherit these.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Base URL for API (e.g. https://coding.dashscope.aliyuncs.com/v1).
    pub base_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    /// Env var for API key (e.g. CODING_PLAN_API_KEY).
    #[serde(default)]
    pub api_key_env: Option<String>,
    /// API format: "openai_compatible" or "anthropic".
    #[serde(default = "default_provider_api")]
    pub api: String,
}

fn default_provider_api() -> String {
    "openai_compatible".to_string()
}

/// Raw model config from TOML. When provider_id is set, inherits from provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawModelEntry {
    /// Provider id. When set, inherits base_url, api_key_env, api from provider.
    #[serde(default)]
    pub provider_id: Option<String>,
    /// Model id (used in API). With provider_id, effective id becomes "provider_id/id".
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub api_base: Option<String>,
    #[serde(default)]
    pub context_window: u64,
    #[serde(default)]
    pub max_tokens: u32,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub enable_search: bool,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
}

/// Resolved model entry used at runtime. Built from RawModelEntry + ProviderConfig.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    /// Unique id for switching (e.g. "qwen-plus", "dashscope/qwen3.5-plus", "coding_plan/kimi-k2.5").
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub api_base: Option<String>,
    #[serde(default)]
    pub context_window: u64,
    #[serde(default)]
    pub max_tokens: u32,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub enable_search: bool,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
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
    /// Providers: id -> config. Models with provider_id inherit from here.
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
    /// Models. With provider_id, inherits base_url/api_key_env/api from provider.
    #[serde(default)]
    pub models: Vec<RawModelEntry>,
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
                providers: HashMap::new(),
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

    /// Returns the list of available models. Resolves provider hierarchy when provider_id is set.
    pub fn list_models(&self) -> Vec<ModelEntry> {
        if self.llm.models.is_empty() {
            let name = if self.llm.model.is_empty() {
                "default".to_string()
            } else {
                self.llm.model.clone()
            };
            return vec![ModelEntry {
                id: self.llm.model.clone(),
                name: name.clone(),
                provider: self.llm.provider.clone(),
                model: self.llm.model.clone(),
                api_base: self.llm.api_base.clone(),
                context_window: self.llm.context_window,
                max_tokens: self.llm.max_tokens,
                tools: vec![],
                enable_search: false,
                api_key: None,
                api_key_env: None,
            }];
        }
        let mut result = Vec::new();
        for raw in &self.llm.models {
            let entry = if let Some(ref pid) = raw.provider_id {
                let prov = match self.llm.providers.get(pid) {
                    Some(p) => p,
                    None => continue,
                };
                ModelEntry {
                    id: format!("{}/{}", pid, raw.id),
                    name: if raw.name.is_empty() {
                        raw.model.clone()
                    } else {
                        raw.name.clone()
                    },
                    provider: prov.api.clone(),
                    model: raw.model.clone(),
                    api_base: Some(prov.base_url.clone()),
                    context_window: if raw.context_window > 0 {
                        raw.context_window
                    } else {
                        self.llm.context_window
                    },
                    max_tokens: if raw.max_tokens > 0 {
                        raw.max_tokens
                    } else {
                        self.llm.max_tokens
                    },
                    tools: raw.tools.clone(),
                    enable_search: raw.enable_search,
                    api_key: raw.api_key.clone().or(prov.api_key.clone()),
                    api_key_env: raw.api_key_env.clone().or(prov.api_key_env.clone()),
                }
            } else {
                ModelEntry {
                    id: raw.id.clone(),
                    name: if raw.name.is_empty() {
                        raw.model.clone()
                    } else {
                        raw.name.clone()
                    },
                    provider: if raw.provider.is_empty() {
                        self.llm.provider.clone()
                    } else {
                        raw.provider.clone()
                    },
                    model: raw.model.clone(),
                    api_base: raw.api_base.clone(),
                    context_window: if raw.context_window > 0 {
                        raw.context_window
                    } else {
                        self.llm.context_window
                    },
                    max_tokens: if raw.max_tokens > 0 {
                        raw.max_tokens
                    } else {
                        self.llm.max_tokens
                    },
                    tools: raw.tools.clone(),
                    enable_search: raw.enable_search,
                    api_key: raw.api_key.clone(),
                    api_key_env: raw.api_key_env.clone(),
                }
            };
            result.push(entry);
        }
        result
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

    /// Get API key for a model. Uses per-model api_key/api_key_env when set, else [llm] defaults.
    pub fn api_key_for_model(&self, model_id: &str) -> Result<String> {
        let entry = self.get_model_entry(model_id);
        if let Some(ref e) = entry {
            if let Some(ref key) = e.api_key {
                if !key.is_empty() {
                    return Ok(key.clone());
                }
            }
            if let Some(ref env) = e.api_key_env {
                if !env.is_empty() {
                    return std::env::var(env).with_context(|| {
                        format!(
                            "API key for model '{}' not found. Set env: export {}=your-key",
                            model_id, env
                        )
                    });
                }
            }
        }
        self.api_key()
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

    #[test]
    fn test_api_key_for_model() {
        let toml = r#"
[llm]
provider = "openai_compatible"
model = "qwen-plus"
api_key = "global-key"
api_key_env = "LLM_API_KEY"
max_tokens = 4096

[[llm.models]]
id = "model-with-key"
name = "Model With Key"
provider = "openai_compatible"
model = "model-with-key"
api_key = "per-model-key"

[[llm.models]]
id = "model-no-override"
name = "Model No Override"
provider = "openai_compatible"
model = "model-no-override"

[agent]
max_iterations = 20
system_prompt = "You are a helpful assistant."

[tools]
enabled = ["read_file", "write_file"]
"#;
        let config: AppConfig = toml::from_str(toml).unwrap();

        // Model with api_key uses it directly
        assert_eq!(
            config.api_key_for_model("model-with-key").unwrap(),
            "per-model-key"
        );

        // Model without api_key/api_key_env falls back to [llm] api_key
        assert_eq!(
            config.api_key_for_model("model-no-override").unwrap(),
            "global-key"
        );
    }

    #[test]
    fn test_provider_hierarchy() {
        let toml = r#"
[llm]
provider = "openai_compatible"
model = "qwen-plus"
api_key_env = "LLM_API_KEY"
max_tokens = 4096

[llm.providers.dashscope]
base_url = "https://dashscope.aliyuncs.com/compatible-mode/v1"
api_key_env = "LLM_API_KEY"
api = "openai_compatible"

[llm.providers.coding_plan]
base_url = "https://coding.dashscope.aliyuncs.com/v1"
api_key_env = "CODING_PLAN_API_KEY"
api = "openai_compatible"

[[llm.models]]
provider_id = "dashscope"
id = "qwen-plus"
name = "Qwen Plus"
model = "qwen-plus"
context_window = 131072
max_tokens = 4096

[[llm.models]]
provider_id = "coding_plan"
id = "qwen3.5-plus"
name = "Qwen 3.5 Plus"
model = "qwen3.5-plus"
context_window = 1048576
max_tokens = 65536
enable_search = true

[[llm.models]]
provider_id = "coding_plan"
id = "kimi-k2.5"
name = "Kimi K2.5"
model = "kimi-k2.5"
context_window = 262144
max_tokens = 32768

[agent]
max_iterations = 20
system_prompt = "You are a helpful assistant."

[tools]
enabled = ["read_file", "write_file"]
"#;
        let config: AppConfig = toml::from_str(toml).unwrap();
        let models = config.list_models();
        assert_eq!(models.len(), 3);

        let qwen_plus = models
            .iter()
            .find(|m| m.id == "dashscope/qwen-plus")
            .unwrap();
        assert_eq!(qwen_plus.model, "qwen-plus");
        assert_eq!(
            qwen_plus.api_base.as_deref(),
            Some("https://dashscope.aliyuncs.com/compatible-mode/v1")
        );
        assert_eq!(qwen_plus.api_key_env.as_deref(), Some("LLM_API_KEY"));

        let qwen35 = models
            .iter()
            .find(|m| m.id == "coding_plan/qwen3.5-plus")
            .unwrap();
        assert_eq!(qwen35.model, "qwen3.5-plus");
        assert_eq!(
            qwen35.api_base.as_deref(),
            Some("https://coding.dashscope.aliyuncs.com/v1")
        );
        assert_eq!(qwen35.api_key_env.as_deref(), Some("CODING_PLAN_API_KEY"));
        assert!(qwen35.enable_search);

        let kimi = models
            .iter()
            .find(|m| m.id == "coding_plan/kimi-k2.5")
            .unwrap();
        assert_eq!(kimi.model, "kimi-k2.5");
        assert_eq!(kimi.context_window, 262144);
    }
}
