//! UI Module - Pluggable user interface layer for miniclaw.
//!
//! This module provides an abstraction over different user interfaces:
//! - Terminal (current implementation)
//! - Web interface (future)
//! - Desktop GUI (future)
//! - VS Code extension (future)
//!
//! Key concepts:
//! - **UI trait**: Defines the interface all UI implementations must follow
//! - **Pluggable**: The core agent doesn't care about the specific UI
//! - **Event-driven**: UI communicates with agent via events

use anyhow::Result;
use async_trait::async_trait;

use crate::agent::Agent;

/// Event types that flow between UI and Agent
#[derive(Debug)]
pub enum UiEvent {
    /// User entered a message
    UserInput(String),
    /// Agent produced a response
    AgentResponse(String),
    /// Agent is processing (for showing loading states)
    AgentProcessing,
    /// Tool is being executed (for showing progress)
    ToolExecution { tool_name: String, args: String },
    /// Error occurred
    Error(String),
    /// UI command (like /clear, /quit)
    Command(String),
}

/// What should happen when a UI exits its run loop.
#[derive(Debug, Clone)]
pub enum UiExitAction {
    /// User wants to quit the application entirely.
    Quit,
    /// User wants to switch to another UI.
    SwitchUi(String),
}

/// Trait that all UI implementations must follow.
#[async_trait]
pub trait Ui: Send {
    /// Start the UI and begin processing events.
    /// Returns the agent back (so another UI can take over) plus the exit action.
    async fn run(&mut self, agent: Agent) -> Result<(Agent, UiExitAction)>;

    /// Send an event from the agent to the UI.
    async fn send_event(&mut self, event: UiEvent) -> Result<()>;

    /// Receive an event from the UI to send to the agent.
    async fn recv_event(&mut self) -> Result<UiEvent>;
}

/// Terminal UI implementation (wraps the current CLI functionality)
pub mod terminal_ui {
    use super::*;
    use crate::cli;

    pub struct TerminalUi;

    #[async_trait]
    impl Ui for TerminalUi {
        async fn run(&mut self, agent: Agent) -> Result<(Agent, UiExitAction)> {
            cli::run_chat_loop(agent).await
        }

        async fn send_event(&mut self, _event: UiEvent) -> Result<()> {
            // Terminal UI handles its own rendering
            Ok(())
        }

        async fn recv_event(&mut self) -> Result<UiEvent> {
            // For now, just return a dummy event
            Ok(UiEvent::UserInput("".to_string()))
        }
    }
}

/// Ratatui-based modern terminal UI
pub mod ratatui_ui;

// Future UI implementations would go here:
/*
/// Simple enhanced terminal UI without external dependencies
pub mod simple_tui {
    use super::*;

    pub struct SimpleTui {
        input: String,
        messages: Vec<String>,
        processing: bool,
    }

    impl SimpleTui {
        pub fn new() -> Self {
            Self {
                input: String::new(),
                messages: vec!["Welcome to miniclaw! Start a conversation by typing below.".to_string()],
                processing: false,
            }
        }
    }

    #[async_trait]
    impl Ui for SimpleTui {
        async fn run(&mut self, mut agent: Agent) -> Result<()> {
            println!("========================================");
            println!("    Enhanced miniclaw UI (Simple TUI)");
            println!("========================================");
            println!("Commands: Type your message and press Enter. Use /quit to exit.\n");

            loop {
                print!("> ");
                std::io::stdout().flush()?;

                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                let input = input.trim().to_string();

                if input == "/quit" || input == "/exit" {
                    break;
                }

                if input.is_empty() {
                    continue;
                }

                // Process with agent
                match agent.process_message(&input).await {
                    Ok(response) => {
                        println!("Assistant: {}", response);
                    }
                    Err(e) => {
                        println!("Error: {}", e);
                    }
                }
            }

            Ok(())
        }

        async fn send_event(&mut self, event: UiEvent) -> Result<()> {
            match event {
                UiEvent::UserInput(input) => {
                    self.messages.push(format!("You: {}", input));
                }
                UiEvent::AgentResponse(response) => {
                    self.messages.push(format!("Assistant: {}", response));
                }
                UiEvent::AgentProcessing => {
                    self.processing = true;
                }
                UiEvent::Error(error_msg) => {
                    self.messages.push(format!("Error: {}", error_msg));
                }
                _ => {} // Ignore other event types for now
            }
            Ok(())
        }

        async fn recv_event(&mut self) -> Result<UiEvent> {
            Ok(UiEvent::UserInput(self.input.clone()))
        }
    }
}

*/