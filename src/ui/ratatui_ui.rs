//! Modern TUI (Terminal User Interface) implementation using ratatui
//!
//! This creates a more sophisticated terminal interface with:
//! - Split screen layout (conversation history + input area)
//! - Syntax highlighting for code blocks
//! - Better formatting of messages
//! - Visual indicators for processing states
//! - Scrollable conversation history

use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::{
    agent::Agent,
    ui::{Ui, UiEvent},
};

/// State management for the TUI application
pub struct RatatuiUi {
    input: String,
    cursor_position: usize,
    messages: Vec<String>,
    scroll_offset: usize,
    processing: bool,
}

impl RatatuiUi {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            cursor_position: 0,
            messages: vec!["Welcome to miniclaw! Start a conversation by typing below.".to_string()],
            scroll_offset: 0,
            processing: false,
        }
    }

    /// Handle key events for input
    fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(c) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    match c {
                        'u' => {
                            // Clear entire input
                            self.input.clear();
                            self.cursor_position = 0;
                        }
                        'k' => {
                            // Clear from cursor to end
                            self.input.drain(self.cursor_position..);
                        }
                        'w' => {
                            // Delete word before cursor
                            let prev_cursor = self.cursor_position;
                            self.move_cursor_start_of_word();
                            self.input.drain(self.cursor_position..prev_cursor);
                        }
                        _ => {}
                    }
                } else {
                    self.input.insert(self.cursor_position, c);
                    self.cursor_position += 1;
                }
            }
            KeyCode::Backspace => {
                if self.cursor_position > 0 {
                    self.input.remove(self.cursor_position - 1);
                    self.cursor_position -= 1;
                }
            }
            KeyCode::Delete => {
                if self.cursor_position < self.input.len() {
                    self.input.remove(self.cursor_position);
                }
            }
            KeyCode::Left => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                }
            }
            KeyCode::Right => {
                if self.cursor_position < self.input.len() {
                    self.cursor_position += 1;
                }
            }
            KeyCode::Home => {
                self.cursor_position = 0;
            }
            KeyCode::End => {
                self.cursor_position = self.input.len();
            }
            _ => {}
        }
    }

    /// Move cursor to start of current word
    fn move_cursor_start_of_word(&mut self) {
        let chars: Vec<char> = self.input.chars().collect();
        while self.cursor_position > 0 {
            if self.cursor_position > 0 {
                self.cursor_position -= 1;
            }
            if self.cursor_position == 0 {
                break;
            }
            if chars[self.cursor_position - 1].is_whitespace() {
                break;
            }
        }
    }

    /// Render the conversation history panel
    fn render_conversation(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        // Combine all messages into a single text
        let mut text_lines = Vec::new();

        for msg in &self.messages {
            // Simple parsing to format differently based on sender
            if msg.starts_with("You:") {
                text_lines.push(Line::from(vec![
                    Span::styled("You: ", Style::default().fg(Color::Green)),
                    Span::raw(&msg[4..]) // Remove "You: " prefix
                ]));
            } else if msg.starts_with("Assistant:") {
                text_lines.push(Line::from(vec![
                    Span::styled("Assistant: ", Style::default().fg(Color::Blue)),
                    Span::raw(&msg[12..]) // Remove "Assistant: " prefix
                ]));
            } else {
                text_lines.push(Line::from(msg.as_str()));
            }

            // Add empty line between messages
            text_lines.push(Line::from(""));
        }

        // Create paragraph widget for messages
        let messages_paragraph = Paragraph::new(text_lines)
            .block(Block::default().borders(Borders::ALL).title("Conversation"))
            .wrap(Wrap { trim: true })
            .scroll((self.scroll_offset as u16, 0));

        f.render_widget(messages_paragraph, area);
    }

    /// Render the input area
    fn render_input(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        // Create input field with indicator
        let (input_text, cursor_idx) = if self.processing {
            ("Processing... Please wait.", 0)
        } else {
            (&self.input[..], self.cursor_position)
        };

        let input_field = Paragraph::new(input_text)
            .block(Block::default().borders(Borders::ALL).title("Input (Press Ctrl+C to quit)"));

        f.render_widget(input_field, area);

        // Set cursor position
        if !self.processing {
            f.set_cursor_position((area.x + cursor_idx as u16 + 1, area.y + 1));
        }
    }

    /// Render the status bar
    fn render_status(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let status_text = if self.processing {
            Line::from(vec![
                Span::styled("PROCESSING", Style::default().bg(Color::Yellow).fg(Color::Black)),
                Span::raw(" - Assistant is thinking...")
            ])
        } else {
            Line::from(vec![
                Span::styled("READY", Style::default().bg(Color::Green).fg(Color::White)),
                Span::raw(" - Press Enter to submit, Ctrl+C to quit")
            ])
        };

        let status_bar = Paragraph::new(status_text)
            .style(Style::default().bg(Color::DarkGray).fg(Color::White))
            .alignment(ratatui::layout::Alignment::Left);

        f.render_widget(status_bar, area);
    }
}

#[async_trait]
impl Ui for RatatuiUi {
    async fn run(&mut self, mut agent: Agent) -> Result<()> {
        // Setup terminal using ratatui's built-in init
        let mut terminal = ratatui::init();

        // Main event loop
        loop {
            terminal.draw(|f| {
                // Define layout: status bar (top), conversation (middle), input (bottom)
                let chunks = Layout::vertical([
                    Constraint::Length(3),  // Status bar
                    Constraint::Min(10),    // Conversation
                    Constraint::Length(3),  // Input area
                ]).split(f.area());

                self.render_status(f, chunks[0]);
                self.render_conversation(f, chunks[1]);
                self.render_input(f, chunks[2]);
            })?;

            if event::poll(std::time::Duration::from_millis(10))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Enter => {
                            if !self.input.trim().is_empty() && !self.processing {
                                // Process the input
                                let user_input = self.input.clone();

                                // Add to messages
                                self.messages.push(format!("You: {}", user_input));

                                // Clear input
                                self.input.clear();
                                self.cursor_position = 0;

                                // Indicate processing
                                self.processing = true;

                                // Process with agent
                                match agent.process_message(&user_input).await {
                                    Ok(response) => {
                                        self.messages.push(format!("Assistant: {}", response));
                                    }
                                    Err(e) => {
                                        self.messages.push(format!("Error: {}", e));
                                    }
                                }

                                self.processing = false;

                                // Scroll to bottom
                                self.scroll_offset = self.messages.len().saturating_sub(
                                    (terminal.get_frame().area().height as usize).saturating_sub(7)
                                );
                            }
                        }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            break; // Exit on Ctrl+C
                        }
                        KeyCode::Up => {
                            if self.scroll_offset > 0 {
                                self.scroll_offset -= 1;
                            }
                        }
                        KeyCode::Down => {
                            let max_scroll = self.messages.len().saturating_sub(
                                (terminal.get_frame().area().height as usize).saturating_sub(7)
                            );
                            if self.scroll_offset < max_scroll {
                                self.scroll_offset += 1;
                            }
                        }
                        _ => {
                            if !self.processing {
                                self.handle_key_event(key);
                            }
                        }
                    }
                }
            }
        }

        // Restore terminal
        ratatui::restore();

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
        // This implementation doesn't use recv_event in the main loop
        // since it handles input directly in the run method
        Ok(UiEvent::UserInput(self.input.clone()))
    }
}