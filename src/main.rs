mod agent;
mod config;
mod llm;
mod rules;
mod session;
mod tools;
mod transport;
mod trusted_workspaces;
mod types;
mod ui;

use anyhow::Result;
use clap::Parser;
use config::AppConfig;
use transport::{resolve_mode, Args};

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let config_path = AppConfig::config_path()?;
    if !config_path.exists() {
        let path = AppConfig::save_default()?;
        eprintln!("[Config] Created default config: {}", path.display());
        eprintln!("[Config] Edit it to set your api_key, model, etc.");
    }

    let config = AppConfig::load()?;
    let mode = resolve_mode(&args);

    match mode {
        transport::ResolvedMode::Tui => {
            let project_root = std::env::current_dir().unwrap_or_default();
            let agent = agent::Agent::create(&config, &project_root)?;
            let tui = ui::ratatui_ui::RatatuiUi::new(config.clone(), project_root);
            let _exit = tui.run(agent).await?;
        }
        transport::ResolvedMode::Cli(cli_args) => {
            transport::cli::run_cli(cli_args, config).await?;
        }
        #[cfg(feature = "telegram")]
        transport::ResolvedMode::Telegram(tg_args) => {
            transport::telegram::run_telegram(tg_args, config).await?;
        }
        #[cfg(not(feature = "telegram"))]
        transport::ResolvedMode::TelegramStub => {
            eprintln!("Telegram support requires building with --features telegram:");
            eprintln!("  cargo build --features telegram");
            std::process::exit(1);
        }
    }

    Ok(())
}
