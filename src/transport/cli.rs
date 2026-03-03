//! CLI transport: one-shot or interactive mode.
//!
//! - One-shot: `miniclaw cli --message "hello"` or `miniclaw --message "hello"`
//! - Interactive: `miniclaw cli` - read from stdin line by line

use anyhow::Result;
use clap::Args;
use std::io::{self, BufRead, Write};

use crate::agent::Agent;
use crate::config::AppConfig;

#[derive(Args, Debug, Clone)]
pub struct CliArgs {
    /// Message to send (one-shot mode). If omitted, runs interactive mode.
    #[arg(short, long)]
    pub message: Option<String>,

    /// Interactive mode: read messages from stdin line by line
    #[arg(short, long, default_value_t = false)]
    pub interactive: bool,
}

pub async fn run_cli(args: CliArgs, config: AppConfig) -> Result<()> {
    let project_root = std::env::current_dir().unwrap_or_default();
    let mut agent = Agent::create(&config, &project_root)?;

    if let Some(msg) = args.message {
        run_one_shot(&mut agent, &msg).await?;
        return Ok(());
    }

    run_interactive(&mut agent).await
}

async fn run_one_shot(agent: &mut Agent, message: &str) -> Result<()> {
    let result = agent.process_message(message, None, None).await?;
    println!("{}", result);
    Ok(())
}

async fn run_interactive(agent: &mut Agent) -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut lines = stdin.lock().lines();

    eprintln!("miniclaw CLI (interactive). Type your message and press Enter. Ctrl+D to exit.");
    eprintln!();

    loop {
        write!(stdout, "> ")?;
        stdout.flush()?;

        let Some(Ok(line)) = lines.next() else {
            break;
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let result = agent.process_message(line, None, None).await?;
        println!("{}", result);
        println!();
    }

    Ok(())
}
