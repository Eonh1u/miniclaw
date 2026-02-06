mod agent;
mod cli;
mod config;
mod llm;
mod tools;
mod types;

use anyhow::Result;
use config::AppConfig;
use llm::anthropic::AnthropicProvider;
use tools::create_default_router;

#[tokio::main]
async fn main() -> Result<()> {
    println!("========================================");
    println!("  miniclaw - AI Assistant (v0.1.0)");
    println!("========================================");

    let config = AppConfig::load()?;
    let api_key = config.api_key()?;
    let llm_provider = Box::new(AnthropicProvider::new(api_key));
    let tool_router = create_default_router();
    let agent = agent::Agent::new(llm_provider, tool_router, config);
    println!("[Agent] Ready!");

    cli::run_chat_loop(agent).await
}
