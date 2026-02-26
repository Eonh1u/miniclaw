mod agent;
mod config;
mod llm;
mod rules;
mod session;
mod tools;
mod types;
mod ui;

use anyhow::Result;
use config::AppConfig;

#[tokio::main]
async fn main() -> Result<()> {
    let config_path = AppConfig::config_path()?;
    if !config_path.exists() {
        let path = AppConfig::save_default()?;
        eprintln!("[Config] Created default config: {}", path.display());
        eprintln!("[Config] Edit it to set your api_key, model, etc.");
    }

    let config = AppConfig::load()?;
    let project_root = std::env::current_dir().unwrap_or_default();
    let agent = agent::Agent::create(&config, &project_root)?;

    let tui = ui::ratatui_ui::RatatuiUi::new(config.clone(), project_root);
    let _exit = tui.run(agent).await?;

    Ok(())
}
