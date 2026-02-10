mod agent;
mod cli;
mod config;
mod llm;
mod tools;
mod types;
mod ui;

use anyhow::{bail, Result};
use config::AppConfig;
use llm::LlmProvider;
use llm::anthropic::AnthropicProvider;
use llm::openai_compatible::OpenAiCompatibleProvider;
use tools::create_default_router;
use ui::Ui;

/// Create the LLM provider based on config.
fn create_llm_provider(config: &AppConfig) -> Result<Box<dyn LlmProvider>> {
    let api_key = config.api_key()?;
    let api_base = config.llm.api_base.clone();

    match config.llm.provider.as_str() {
        "anthropic" => {
            Ok(Box::new(AnthropicProvider::new(api_key, api_base)))
        }
        "openai_compatible" | "openai" => {
            Ok(Box::new(OpenAiCompatibleProvider::new(api_key, api_base)))
        }
        other => {
            bail!(
                "Unknown provider: '{}'. Supported: 'anthropic', 'openai_compatible'",
                other
            )
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("========================================");
    println!("  miniclaw - AI Assistant (v0.1.0)");
    println!("========================================");

    // Auto-generate config file on first run
    let config_path = AppConfig::config_path()?;
    if !config_path.exists() {
        let path = AppConfig::save_default()?;
        println!("[Config] Created default config: {}", path.display());
        println!("[Config] Edit it to set your api_key, model, etc.");
    }

    let config = AppConfig::load()?;
    println!(
        "[Config] Provider: {}, Model: {}, API: {}",
        config.llm.provider,
        config.llm.model,
        config.llm.api_base.as_deref().unwrap_or("(default)")
    );

    let llm_provider = create_llm_provider(&config)?;
    let tool_router = create_default_router();
    let agent = agent::Agent::new(llm_provider, tool_router, config);
    println!("[Agent] Ready!");

    // Determine UI type based on command-line arguments or environment
    let ui_type = std::env::var("MINICLAW_UI")
        .unwrap_or_else(|_| "terminal".to_string())
        .to_lowercase();

    match ui_type.as_str() {
        "ratatui" | "tui" | "modern" => {
            let mut ui = ui::ratatui_ui::RatatuiUi::new();
            ui.run(agent).await?;
        }
        "terminal" | "simple" | "cli" => {
            let mut ui = ui::terminal_ui::TerminalUi {};
            ui.run(agent).await?;
        }
        _ => {
            println!("Unknown UI type: {}, using terminal UI", ui_type);
            let mut ui = ui::terminal_ui::TerminalUi {};
            ui.run(agent).await?;
        }
    }

    Ok(())
}
