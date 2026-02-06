//! CLI Chat Interface with interactive command selection.
//!
//! Key concepts:
//! - **Helper**: rustyline's trait for Completer + Hinter + Highlighter + Validator
//! - **crossterm**: low-level terminal control for building interactive menus
//! - **Raw mode**: when showing the menu, we switch the terminal to raw mode
//!   so we can read individual keypresses (arrow keys, Enter, Esc)

use std::io::{self, Write};

use anyhow::Result;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    style::{self, Stylize},
    terminal::{self, ClearType},
};
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::{Hint, Hinter};
use rustyline::validate::Validator;
use rustyline::{Context, Editor, Helper};

use crate::agent::Agent;

// --- Command definitions ---

struct Command {
    name: &'static str,
    description: &'static str,
}

const COMMANDS: &[Command] = &[
    Command { name: "/help",  description: "Show available commands" },
    Command { name: "/clear", description: "Clear conversation history" },
    Command { name: "/quit",  description: "Exit the program" },
];

// --- Interactive command menu ---

/// Show an interactive command selection menu using crossterm.
/// Returns the selected command name, or None if the user pressed Esc.
fn show_command_menu() -> Result<Option<String>> {
    let mut stdout = io::stdout();
    let mut selected: usize = 0;
    let total = COMMANDS.len();

    // Enter raw mode so we can capture individual keypresses
    terminal::enable_raw_mode()?;

    // Draw the menu
    let draw = |stdout: &mut io::Stdout, sel: usize| -> Result<()> {
        for (i, cmd) in COMMANDS.iter().enumerate() {
            execute!(stdout, cursor::MoveToColumn(0))?;
            if i == sel {
                // Highlighted item: reverse colors
                let line = format!("  > {:10} {}", cmd.name, cmd.description);
                execute!(stdout, style::PrintStyledContent(line.reverse()))?;
            } else {
                let line = format!("    {:10} {}", cmd.name, cmd.description);
                execute!(stdout, style::PrintStyledContent(line.stylize()))?;
            }
            execute!(stdout, terminal::Clear(ClearType::UntilNewLine))?;
            if i < total - 1 {
                execute!(stdout, style::Print("\r\n"))?;
            }
        }
        // Move cursor back to top of menu
        if total > 1 {
            execute!(stdout, cursor::MoveUp((total - 1) as u16))?;
        }
        stdout.flush()?;
        Ok(())
    };

    // Initial draw
    draw(&mut stdout, selected)?;

    // Event loop
    let result = loop {
        if let Event::Key(KeyEvent { code, .. }) = event::read()? {
            match code {
                KeyCode::Up => {
                    if selected > 0 {
                        selected -= 1;
                    } else {
                        selected = total - 1; // Wrap around
                    }
                    draw(&mut stdout, selected)?;
                }
                KeyCode::Down => {
                    if selected < total - 1 {
                        selected += 1;
                    } else {
                        selected = 0; // Wrap around
                    }
                    draw(&mut stdout, selected)?;
                }
                KeyCode::Enter => {
                    break Some(COMMANDS[selected].name.to_string());
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    break None;
                }
                _ => {}
            }
        }
    };

    // Clean up: move to bottom of menu, clear, restore normal mode
    execute!(stdout, cursor::MoveToColumn(0))?;
    for _ in 0..total {
        execute!(
            stdout,
            terminal::Clear(ClearType::CurrentLine),
            style::Print("\r\n")
        )?;
    }
    // Move back up to overwrite menu lines
    execute!(stdout, cursor::MoveUp(total as u16))?;
    for _ in 0..total {
        execute!(
            stdout,
            terminal::Clear(ClearType::CurrentLine),
            cursor::MoveDown(1)
        )?;
    }
    execute!(stdout, cursor::MoveUp(total as u16))?;
    execute!(stdout, cursor::MoveToColumn(0))?;

    terminal::disable_raw_mode()?;

    Ok(result)
}

// --- Hint type for rustyline ---

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
        if !line.starts_with('/') {
            return Ok((pos, vec![]));
        }

        let input = &line[..pos];
        let matches: Vec<Pair> = COMMANDS
            .iter()
            .filter(|cmd| cmd.name.starts_with(input) && cmd.name != input)
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
        if !line.starts_with('/') || pos < 2 {
            return None;
        }

        let input = &line[..pos];
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

// --- Execute a command ---

/// Execute a slash command. Returns true if the loop should break (exit).
fn execute_command(cmd: &str, agent: &mut Agent) -> bool {
    match cmd {
        "/quit" => {
            println!("Goodbye!");
            return true;
        }
        "/clear" => {
            agent.clear_history();
            println!("[Conversation cleared]");
        }
        "/help" => {
            println!();
            println!("Available commands (type / to select interactively):");
            for c in COMMANDS {
                println!("  {:10} {}", c.name, c.description);
            }
            println!();
        }
        other => {
            println!("[Unknown command: {}. Type / to see available commands]", other);
        }
    }
    false
}

// --- Chat loop ---

pub async fn run_chat_loop(mut agent: Agent) -> Result<()> {
    let helper = MiniclawHelper;
    let mut rl = Editor::new()?;
    rl.set_helper(Some(helper));

    println!();
    println!("Type your message, or / to select a command.");
    println!();

    loop {
        match rl.readline("You > ") {
            Ok(line) => {
                let input = line.trim().to_string();
                if input.is_empty() {
                    continue;
                }

                // "/" alone -> show interactive menu
                if input == "/" {
                    match show_command_menu() {
                        Ok(Some(cmd)) => {
                            if execute_command(&cmd, &mut agent) {
                                break;
                            }
                        }
                        Ok(None) => {
                            // User pressed Esc, cancelled
                        }
                        Err(e) => {
                            println!("[Menu error: {}]", e);
                        }
                    }
                    continue;
                }

                // Direct slash command (e.g. /quit typed fully)
                if input.starts_with('/') {
                    if execute_command(&input, &mut agent) {
                        break;
                    }
                    continue;
                }

                // Normal message -> send to agent
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
