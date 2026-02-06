mod agent;
mod cli;
mod config;
mod llm;
mod tools;
mod types;

use anyhow::{bail, Result};
use config::AppConfig;
use llm::LlmProvider;
use llm::anthropic::AnthropicProvider;
use llm::openai_compatible::OpenAiCompatibleProvider;
use tools::create_default_router;

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

    cli::run_chat_loop(agent).await
}
