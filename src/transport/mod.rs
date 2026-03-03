//! Transport layer: routes user input to different channels (TUI, CLI, Telegram).
//!
//! Similar to OpenClaw's channel routing: deterministic routing based on how
//! the user invokes miniclaw.

pub mod cli;

#[cfg(feature = "telegram")]
pub mod telegram;
#[cfg(feature = "telegram")]
mod telegram_state;

use clap::Parser;

/// Miniclaw - A minimal AI assistant inspired by OpenClaw
#[derive(Parser, Debug)]
#[command(name = "miniclaw")]
#[command(about = "Terminal AI assistant with TUI, CLI, and Telegram support")]
pub struct Args {
    /// Subcommand / mode. Default: tui (interactive TUI)
    #[command(subcommand)]
    pub subcommand: Option<Subcommand>,

    /// Legacy: pass message directly for one-shot CLI (same as `cli --message "..."`)
    #[arg(short, long)]
    pub message: Option<String>,
}

#[derive(Parser, Debug, Clone)]
pub enum Subcommand {
    /// Interactive TUI (default)
    Tui,

    /// CLI mode: one-shot or interactive stdin
    Cli(cli::CliArgs),

    /// Run as Telegram bot (requires: cargo build --features telegram)
    #[cfg(feature = "telegram")]
    Telegram(telegram::TelegramArgs),

    /// Run as Telegram bot (stub when telegram feature disabled)
    #[cfg(not(feature = "telegram"))]
    Telegram(TelegramStubArgs),
}

#[cfg(not(feature = "telegram"))]
#[derive(Parser, Debug, Clone)]
pub struct TelegramStubArgs {}

impl Default for Subcommand {
    fn default() -> Self {
        Subcommand::Tui
    }
}

/// Resolve which mode to run. Handles legacy `--message` flag.
pub fn resolve_mode(args: &Args) -> ResolvedMode {
    if let Some(msg) = &args.message {
        return ResolvedMode::Cli(cli::CliArgs {
            message: Some(msg.clone()),
            interactive: false,
        });
    }
    match &args.subcommand {
        None => ResolvedMode::Tui,
        Some(Subcommand::Tui) => ResolvedMode::Tui,
        Some(Subcommand::Cli(c)) => ResolvedMode::Cli(c.clone()),
        #[cfg(feature = "telegram")]
        Some(Subcommand::Telegram(t)) => ResolvedMode::Telegram(t.clone()),
        #[cfg(not(feature = "telegram"))]
        Some(Subcommand::Telegram(_)) => ResolvedMode::TelegramStub,
    }
}

#[derive(Debug)]
pub enum ResolvedMode {
    Tui,
    Cli(cli::CliArgs),
    #[cfg(feature = "telegram")]
    Telegram(telegram::TelegramArgs),
    #[cfg(not(feature = "telegram"))]
    TelegramStub,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_mode_message() {
        let args = Args {
            subcommand: None,
            message: Some("hello".to_string()),
        };
        let mode = resolve_mode(&args);
        match &mode {
            ResolvedMode::Cli(c) => {
                assert_eq!(c.message.as_deref(), Some("hello"));
                assert!(!c.interactive);
            }
            _ => panic!("expected Cli mode"),
        }
    }

    #[test]
    fn test_resolve_mode_tui_default() {
        let args = Args {
            subcommand: None,
            message: None,
        };
        let mode = resolve_mode(&args);
        match &mode {
            ResolvedMode::Tui => {}
            _ => panic!("expected Tui mode"),
        }
    }

    #[test]
    fn test_resolve_mode_cli_subcommand() {
        let args = Args {
            subcommand: Some(Subcommand::Cli(cli::CliArgs {
                message: Some("test".to_string()),
                interactive: true,
            })),
            message: None,
        };
        let mode = resolve_mode(&args);
        match &mode {
            ResolvedMode::Cli(c) => {
                assert_eq!(c.message.as_deref(), Some("test"));
                assert!(c.interactive);
            }
            _ => panic!("expected Cli mode"),
        }
    }
}
