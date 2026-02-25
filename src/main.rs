mod agent;
mod config;
mod llm;
mod rules;
mod tools;
mod types;
mod ui;

use anyhow::{bail, Result};
use config::AppConfig;
use llm::LlmProvider;
use llm::anthropic::AnthropicProvider;
use llm::openai_compatible::OpenAiCompatibleProvider;
use tools::create_default_router;

fn create_llm_provider(config: &AppConfig) -> Result<Box<dyn LlmProvider>> {
    let api_key = config.api_key()?;
    let api_base = config.llm.api_base.clone();

    match config.llm.provider.as_str() {
        "anthropic" => Ok(Box::new(AnthropicProvider::new(api_key, api_base))),
        "openai_compatible" | "openai" => {
            Ok(Box::new(OpenAiCompatibleProvider::new(api_key, api_base)))
        }
        other => bail!(
            "Unknown provider: '{}'. Supported: 'anthropic', 'openai_compatible'",
            other
        ),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let config_path = AppConfig::config_path()?;
    if !config_path.exists() {
        let path = AppConfig::save_default()?;
        eprintln!("[Config] Created default config: {}", path.display());
        eprintln!("[Config] Edit it to set your api_key, model, etc.");
    }

    let config = AppConfig::load()?;
    let llm_provider = create_llm_provider(&config)?;
    let tool_router = create_default_router();
    let project_root = std::env::current_dir().unwrap_or_default();
    let agent = agent::Agent::new(llm_provider, tool_router, config.clone(), &project_root);

    let tui = ui::ratatui_ui::RatatuiUi::new(&config);
    let (_agent, _exit) = tui.run(agent).await?;

    Ok(())
}
