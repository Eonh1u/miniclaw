//! CLI Chat Interface with command completion and hints.
//!
//! Key concepts:
//! - **Helper**: rustyline's trait to combine Completer + Hinter + Highlighter + Validator
//! - **Completer**: when user presses Tab, suggest matching commands
//! - **Hinter**: show greyed-out suggestion as user types (like fish shell)

use anyhow::Result;
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::{Hinter, Hint};
use rustyline::validate::Validator;
use rustyline::{Context, Editor, Helper};

use crate::agent::Agent;

// --- Command definitions ---

struct Command {
    name: &'static str,
    description: &'static str,
}

const COMMANDS: &[Command] = &[
    Command { name: "/quit",  description: "Exit the program" },
    Command { name: "/exit",  description: "Exit the program" },
    Command { name: "/clear", description: "Clear conversation history" },
    Command { name: "/help",  description: "Show available commands" },
];

// --- Hint type ---

struct CommandHint {
    display: String,
    complete_up_to: usize,
}

impl Hint for CommandHint {
    fn display(&self) -> &str {
        &self.display
    }

    fn completion(&self) -> Option<&str> {
        if self.complete_up_to > 0 {
            Some(&self.display[..self.complete_up_to])
        } else {
            None
        }
    }
}

// --- Helper implementation ---

struct MiniclawHelper;

impl Completer for MiniclawHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        // Only complete commands starting with /
        if !line.starts_with('/') {
            return Ok((pos, vec![]));
        }

        let input = &line[..pos];
        let matches: Vec<Pair> = COMMANDS
            .iter()
            .filter(|cmd| cmd.name.starts_with(input))
            .map(|cmd| Pair {
                display: format!("{} - {}", cmd.name, cmd.description),
                replacement: cmd.name.to_string(),
            })
            .collect();

        Ok((0, matches))
    }
}

impl Hinter for MiniclawHelper {
    type Hint = CommandHint;

    fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<CommandHint> {
        if !line.starts_with('/') || pos < 1 {
            return None;
        }

        let input = &line[..pos];
        // Find the first command that matches
        COMMANDS
            .iter()
            .find(|cmd| cmd.name.starts_with(input) && cmd.name != input)
            .map(|cmd| {
                let suffix = &cmd.name[pos..];
                CommandHint {
                    display: suffix.to_string(),
                    complete_up_to: suffix.len(),
                }
            })
    }
}

impl Highlighter for MiniclawHelper {}
impl Validator for MiniclawHelper {}
impl Helper for MiniclawHelper {}

// --- Print help ---

fn print_help() {
    println!();
    println!("Available commands:");
    for cmd in COMMANDS {
        println!("  {:10} {}", cmd.name, cmd.description);
    }
    println!();
}

// --- Chat loop ---

pub async fn run_chat_loop(mut agent: Agent) -> Result<()> {
    let helper = MiniclawHelper;
    let mut rl = Editor::new()?;
    rl.set_helper(Some(helper));

    println!();
    println!("Type your message, or / for commands (Tab to complete).");
    println!("Type /help to see all commands.");
    println!();

    loop {
        match rl.readline("You > ") {
            Ok(line) => {
                let input = line.trim().to_string();
                if input.is_empty() { continue; }

                match input.as_str() {
                    "/quit" | "/exit" => {
                        println!("Goodbye!");
                        break;
                    }
                    "/clear" => {
                        agent.clear_history();
                        println!("[Conversation cleared]");
                        continue;
                    }
                    "/help" => {
                        print_help();
                        continue;
                    }
                    s if s.starts_with('/') => {
                        println!("[Unknown command: {}. Type /help for available commands]", s);
                        continue;
                    }
                    _ => {}
                }

                let _ = rl.add_history_entry(&input);

                println!();
                match agent.process_message(&input).await {
                    Ok(r) => println!("Assistant > {}\n", r),
                    Err(e) => println!("[Error: {}]\n", e),
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("\nGoodbye!");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("Goodbye!");
                break;
            }
            Err(err) => {
                println!("[Input error: {}]", err);
                break;
            }
        }
    }
    Ok(())
}
