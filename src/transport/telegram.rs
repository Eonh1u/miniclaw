//! Telegram transport: run miniclaw as a Telegram bot.
//!
//! Requires `telegram` feature: `cargo build --features telegram`
//! Config: `[telegram]` section in ~/.miniclaw/config.toml, or TELEGRAM_BOT_TOKEN env.

use anyhow::{Context, Result};
use clap::Args;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::utils::command::BotCommands;

use crate::agent::Agent;
use crate::config::AppConfig;

use super::telegram_state::TelegramStateStore;

#[derive(Args, Debug, Clone)]
pub struct TelegramArgs {
    /// Bot token (overrides config; also use TELEGRAM_BOT_TOKEN env)
    #[arg(long)]
    pub token: Option<String>,
    /// Run as background daemon (spawns child, parent exits)
    #[arg(long)]
    pub daemon: bool,
    /// Stop the running daemon
    #[arg(long)]
    pub stop: bool,
}

fn pid_file_path() -> Result<std::path::PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    Ok(home.join(".miniclaw").join("miniclaw-telegram.pid"))
}

fn run_daemon_stop() -> Result<()> {
    let path = pid_file_path()?;
    if !path.exists() {
        eprintln!("No daemon running (PID file not found: {})", path.display());
        return Ok(());
    }
    let pid_str = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let pid: i32 = pid_str.trim().parse().context("Invalid PID in file")?;

    #[cfg(unix)]
    {
        let err = unsafe { libc::kill(pid, libc::SIGTERM) };
        if err != 0 {
            let e = std::io::Error::last_os_error();
            if e.raw_os_error() == Some(libc::ESRCH) {
                eprintln!("Process {} not found (already stopped?)", pid);
                let _ = std::fs::remove_file(&path);
                return Ok(());
            }
            anyhow::bail!("Failed to send SIGTERM to {}: {}", pid, e);
        }
    }
    #[cfg(not(unix))]
    {
        eprintln!("--stop is only supported on Unix");
        return Ok(());
    }

    eprintln!("Sent SIGTERM to {} (daemon stopping)", pid);
    let _ = std::fs::remove_file(&path);
    Ok(())
}

pub async fn run_telegram(args: TelegramArgs, config: AppConfig) -> Result<()> {
    if args.stop {
        return run_daemon_stop();
    }

    if args.daemon {
        let exe = std::env::current_exe().context("Could not get executable path")?;
        let child: Child = Command::new(&exe)
            .arg("telegram")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("Failed to spawn daemon process")?;

        let pid = child.id();
        let path = pid_file_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
        std::fs::write(&path, pid.to_string())
            .with_context(|| format!("Failed to write PID file {}", path.display()))?;

        eprintln!("miniclaw Telegram daemon started, PID: {}", pid);
        eprintln!("PID file: {}", path.display());
        eprintln!("Stop with: miniclaw telegram --stop");
        return Ok(());
    }

    run_telegram_foreground(config).await
}

async fn run_telegram_foreground(config: AppConfig) -> Result<()> {
    let token = config
        .telegram
        .as_ref()
        .and_then(|t| t.bot_token.clone())
        .or_else(|| std::env::var("TELEGRAM_BOT_TOKEN").ok())
        .context(
            "Telegram bot token required. Set TELEGRAM_BOT_TOKEN env or [telegram] bot_token in config.",
        )?;

    let bot = Bot::new(token);
    let project_root = config
        .telegram
        .as_ref()
        .and_then(|t| t.workspace.as_ref())
        .map(Path::new)
        .unwrap_or(Path::new("."));

    let project_root = if project_root.is_absolute() {
        project_root.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_default()
            .join(project_root)
    };

    let state_store = Arc::new(TelegramStateStore::new()?);

    let handler = Update::filter_message().branch(
        dptree::entry()
            .filter_command::<BotCommand>()
            .endpoint(handle_command)
            .branch(dptree::filter(|msg: Message| msg.text().is_some()).endpoint(handle_message)),
    );

    let mut bot_dispatcher = Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .dependencies(dptree::deps![config, project_root, state_store])
        .build();

    eprintln!("miniclaw Telegram bot started. Send /start to begin.");
    bot_dispatcher.dispatch().await;

    // Clean up PID file when daemon exits (e.g. Ctrl+C)
    if let Ok(path) = pid_file_path() {
        let _ = std::fs::remove_file(&path);
    }

    Ok(())
}

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
enum BotCommand {
    #[command(description = "Start the bot")]
    Start,
    #[command(description = "Help")]
    Help,
    #[command(description = "List or switch model: /model or /model <id>")]
    Model,
}

async fn handle_command(
    bot: Bot,
    msg: Message,
    cmd: BotCommand,
    config: AppConfig,
    state_store: Arc<TelegramStateStore>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let chat_id = msg.chat.id.0;

    match cmd {
        BotCommand::Start => {
            let response = "Hello! I'm miniclaw, an AI assistant. Send me any message and I'll respond.\n\nCommands:\n/help - Show help\n/model - List or switch model";
            bot.send_message(msg.chat.id, response).await?;
        }
        BotCommand::Help => {
            let response = "Commands:\n/start - Start\n/help - This help\n/model - List or switch model (e.g. /model qwen-plus)\n\nJust send a message to chat with me.";
            bot.send_message(msg.chat.id, response).await?;
        }
        BotCommand::Model => {
            let text = msg.text().unwrap_or("");
            let parts: Vec<&str> = text.split_whitespace().collect();
            let response = if parts.len() >= 2 {
                let model_id = parts[1..].join(" ");
                let models = config.list_models();
                if models.iter().any(|m| m.id == model_id) {
                    state_store.set_model(chat_id, model_id.clone()).await?;
                    format!("Switched to model: {}", model_id)
                } else {
                    format!(
                        "Unknown model: {}. Use /model to list available models.",
                        model_id
                    )
                }
            } else {
                let models = config.list_models();
                let current = state_store
                    .get_model(chat_id)
                    .await
                    .unwrap_or_else(|| config.default_model_id());
                let mut lines = vec![
                    format!("Current model: {}", current),
                    "".to_string(),
                    "Available models:".to_string(),
                ];
                for m in &models {
                    let name = if m.name.is_empty() { &m.model } else { &m.name };
                    lines.push(format!("  • {} ({})", m.id, name));
                }
                lines.push("".to_string());
                lines.push("Switch: /model <id>".to_string());
                lines.join("\n")
            };
            bot.send_message(msg.chat.id, response).await?;
        }
    }
    Ok(())
}

async fn handle_message(
    bot: Bot,
    msg: Message,
    config: AppConfig,
    project_root: std::path::PathBuf,
    state_store: Arc<TelegramStateStore>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let text = msg.text().unwrap_or("");
    if text.is_empty() {
        return Ok(());
    }

    let chat_id = msg.chat.id;

    let status_msg = bot.send_message(chat_id, "⏳ Thinking...").await?;

    let model_id = state_store.get_model(chat_id.0).await;

    let mut agent = Agent::create_with_model(&config, &project_root, model_id.as_deref())?;

    let result = agent.process_message(text, None, None).await;

    let response = match result {
        Ok(s) => s,
        Err(e) => format!("Error: {}", e),
    };

    bot.edit_message_text(chat_id, status_msg.id, &response)
        .await?;

    Ok(())
}
