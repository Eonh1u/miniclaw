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
use crossterm::terminal;
use ratatui::{
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::{
    agent::Agent,
    ui::{Ui, UiEvent, UiExitAction},
};

/// Guard that ensures terminal is restored even if the TUI panics or returns an error.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

// ── Pet Animation System ────────────────────────────────────

/// The pet's current mood / state.
#[derive(Debug, Clone, Copy, PartialEq)]
enum PetState {
    Idle,
    Typing,
    TypingFast,
    Thinking,
    Happy,
    Error,
    Sleeping,
}

/// A single animation frame: multiple lines of ASCII art.
type ArtFrame = &'static [&'static str];

impl PetState {
    /// All animation frames for this state.
    /// The renderer cycles through them automatically.
    fn frames(&self) -> &[ArtFrame] {
        match self {
            // Idle: gentle blinking (eyes open most of the time)
            PetState::Idle => &[
                &[r"   /\_/\  ",
                  r"  ( o.o ) ",
                  r"   > ^ <  ",
                  r"  /|   |\ ",
                  r"  (___)   "],
                &[r"   /\_/\  ",
                  r"  ( o.o ) ",
                  r"   > ^ <  ",
                  r"  /|   |\ ",
                  r"   (___)  "],
                &[r"   /\_/\  ",
                  r"  ( -.- ) ",  // blink!
                  r"   > ^ <  ",
                  r"  /|   |\ ",
                  r"  (___)   "],
                &[r"   /\_/\  ",
                  r"  ( o.o ) ",
                  r"   > ^ <  ",
                  r"  /|   |\ ",
                  r"   (___)  "],
            ],
            // Typing (slow): watching the screen
            PetState::Typing => &[
                &[r"   /\_/\  ",
                  r"  ( o.o ) ",
                  r"   >   <  ",
                  r"   _[_]_  ",
                  r"   click  "],
                &[r"   /\_/\  ",
                  r"  ( o.  ) ",
                  r"   >   <  ",
                  r"   _[_]_  ",
                  r"   clack  "],
                &[r"   /\_/\  ",
                  r"  ( .o ) ",
                  r"   >   <  ",
                  r"   _[_]_  ",
                  r"   click  "],
            ],
            // Typing fast: excited!
            PetState::TypingFast => &[
                &[r"   /\_/\  ",
                  r"  ( O.O ) ",
                  r"   >   <  ",
                  r"  _[===]_ ",
                  r"  *CLACK* "],
                &[r"  ~/\_/\~ ",
                  r"  ( O.O ) ",
                  r"   >   <  ",
                  r"  _[===]_ ",
                  r"  *CLICK* "],
                &[r"   /\_/\  ",
                  r"  ( O.O ) ",
                  r"   >   <  ",
                  r"  _[===]_ ",
                  r"  *CLACK* "],
                &[r" ~/\_/\ ~ ",
                  r"  ( O.O ) ",
                  r"   >   <  ",
                  r"  _[===]_ ",
                  r"  *CLICK* "],
            ],
            // Thinking: looking around with thought bubble
            PetState::Thinking => &[
                &[r"   /\_/\  ",
                  r"  ( o.O ) ",
                  r"   > ~ <  ",
                  r"  /|   |\ ",
                  r"    ...   "],
                &[r"   /\_/\  ",
                  r"  ( O.o ) ",
                  r"   > ~ <  ",
                  r"  /|   |\ ",
                  r"   ...    "],
                &[r"   /\_/\  ",
                  r"  ( o.o ) ",
                  r"   > ? <  ",
                  r"  /|   |\ ",
                  r"     ..   "],
                &[r"   /\_/\  ",
                  r"  ( O.O ) ",
                  r"   > ~ <  ",
                  r"  /|   |\ ",
                  r"  . . .   "],
            ],
            // Happy: bouncy with sparkles
            PetState::Happy => &[
                &[r"   /\_/\  ",
                  r"  ( ^.^ ) ",
                  r"   > v <  ",
                  r"  /|   |\ ",
                  r"  * ~ * ~ "],
                &[r"  ~/\_/\  ",
                  r"  ( ^o^ ) ",
                  r"   > v <  ",
                  r"  /|   |\ ",
                  r"  ~ * ~ * "],
                &[r"   /\_/\~ ",
                  r"  ( ^.^ ) ",
                  r"   > v <  ",
                  r"  /|   |\ ",
                  r"  * * ~ ~ "],
                &[r"  ~/\_/\  ",
                  r"  ( ^o^ ) ",
                  r"   > v <  ",
                  r"  /|   |\ ",
                  r"  ~ ~ * * "],
            ],
            // Error: sad, shaking head
            PetState::Error => &[
                &[r"   /\_/\  ",
                  r"  ( T.T ) ",
                  r"   > _ <  ",
                  r"  /|   |\ ",
                  r"   ...    "],
                &[r"   /\_/\  ",
                  r"  ( ;.; ) ",
                  r"   > _ <  ",
                  r"  /|   |\ ",
                  r"    ...   "],
            ],
            // Sleeping: zzZ animation
            PetState::Sleeping => &[
                &[r"   /\_/\  ",
                  r"  ( -.- ) ",
                  r"   > z <  ",
                  r"  /|   |\ ",
                  r"      z   "],
                &[r"   /\_/\  ",
                  r"  ( -.- ) ",
                  r"   > z <  ",
                  r"  /|   |\ ",
                  r"     zZ   "],
                &[r"   /\_/\  ",
                  r"  ( -.- ) ",
                  r"   > z <  ",
                  r"  /|   |\ ",
                  r"    zZz   "],
                &[r"   /\_/\  ",
                  r"  ( -.- ) ",
                  r"   > z <  ",
                  r"  /|   |\ ",
                  r"   zZzZ   "],
            ],
        }
    }

    /// Ticks per animation frame (100ms each tick).
    /// Faster states animate quicker.
    fn ticks_per_frame(&self) -> u32 {
        match self {
            PetState::Idle       => 8,   // ~0.8s per frame (slow, relaxed)
            PetState::Typing     => 4,   // ~0.4s
            PetState::TypingFast => 2,   // ~0.2s (rapid!)
            PetState::Thinking   => 5,   // ~0.5s
            PetState::Happy      => 3,   // ~0.3s (bouncy)
            PetState::Error      => 6,   // ~0.6s
            PetState::Sleeping   => 10,  // ~1.0s (slow breathing)
        }
    }

    /// A short label describing the mood.
    fn label(&self) -> &str {
        match self {
            PetState::Idle       => "Idle",
            PetState::Typing     => "Watching...",
            PetState::TypingFast => "Excited!!",
            PetState::Thinking   => "Thinking...",
            PetState::Happy      => "Happy!",
            PetState::Error      => "Oh no...",
            PetState::Sleeping   => "zzZ...",
        }
    }

    /// Color used for the pet art.
    fn color(&self) -> Color {
        match self {
            PetState::Idle       => Color::White,
            PetState::Typing     => Color::Cyan,
            PetState::TypingFast => Color::Magenta,
            PetState::Thinking   => Color::Yellow,
            PetState::Happy      => Color::Green,
            PetState::Error      => Color::Red,
            PetState::Sleeping   => Color::DarkGray,
        }
    }

    /// Get the art frame for the current animation tick.
    fn current_frame(&self, tick: u32) -> ArtFrame {
        let frames = self.frames();
        let idx = (tick / self.ticks_per_frame()) as usize % frames.len();
        frames[idx]
    }
}

// ── TUI State ───────────────────────────────────────────────

/// Width (in columns) of the pet panel in the header.
const PET_PANEL_WIDTH: u16 = 20;
/// Height of the entire header row (info panel + pet panel).
const HEADER_HEIGHT: u16 = 10;

/// State management for the TUI application
pub struct RatatuiUi {
    input: String,
    /// Cursor position as a **character index** (not byte index).
    cursor_position: usize,
    messages: Vec<String>,
    scroll_offset: usize,
    /// When true, conversation auto-scrolls to show the latest messages.
    follow_tail: bool,
    processing: bool,

    // ── Pet state ──
    /// Current pet mood.
    pet_state: PetState,
    /// Global animation tick (increments every draw cycle, ~100ms).
    anim_tick: u32,
    /// Ticks since the last keypress (for idle/sleep detection).
    idle_ticks: u32,
    /// Typing intensity: increases on keypress, decays each tick.
    /// Used to detect typing speed and switch between Typing/TypingFast.
    typing_intensity: u32,
}

/// Threshold: above this typing_intensity → TypingFast.
const TYPING_FAST_THRESHOLD: u32 = 15;
/// Threshold: above 0 but below fast → Typing.
const TYPING_DECAY_PER_TICK: u32 = 1;
/// Intensity added per keystroke.
const TYPING_BOOST_PER_KEY: u32 = 4;

impl RatatuiUi {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            cursor_position: 0,
            messages: vec!["Welcome to miniclaw! Start a conversation by typing below.".to_string()],
            scroll_offset: 0,
            follow_tail: true,
            processing: false,
            pet_state: PetState::Idle,
            anim_tick: 0,
            idle_ticks: 0,
            typing_intensity: 0,
        }
    }

    // --- UTF-8 safe cursor helpers ---

    /// Convert the current character-based cursor_position to a byte index in self.input.
    fn byte_index(&self) -> usize {
        self.input
            .char_indices()
            .nth(self.cursor_position)
            .map_or(self.input.len(), |(i, _)| i)
    }

    /// Number of characters in the input (NOT byte length).
    fn char_count(&self) -> usize {
        self.input.chars().count()
    }

    /// Calculate the display width (terminal columns) of the input up to the cursor.
    /// ASCII chars = 1 column, CJK / wide chars = 2 columns.
    fn cursor_display_width(&self) -> u16 {
        self.input
            .chars()
            .take(self.cursor_position)
            .map(|c| if c.is_ascii() { 1u16 } else { 2u16 })
            .sum()
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
                            let byte_idx = self.byte_index();
                            self.input.drain(byte_idx..);
                        }
                        'w' => {
                            // Delete word before cursor
                            let end_byte = self.byte_index();
                            self.move_cursor_start_of_word();
                            let start_byte = self.byte_index();
                            self.input.drain(start_byte..end_byte);
                        }
                        _ => {}
                    }
                } else {
                    let byte_idx = self.byte_index();
                    self.input.insert(byte_idx, c);
                    self.cursor_position += 1;
                }
            }
            KeyCode::Backspace => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                    let byte_idx = self.byte_index();
                    self.input.remove(byte_idx);
                }
            }
            KeyCode::Delete => {
                if self.cursor_position < self.char_count() {
                    let byte_idx = self.byte_index();
                    self.input.remove(byte_idx);
                }
            }
            KeyCode::Left => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                }
            }
            KeyCode::Right => {
                if self.cursor_position < self.char_count() {
                    self.cursor_position += 1;
                }
            }
            KeyCode::Home => {
                self.cursor_position = 0;
            }
            KeyCode::End => {
                self.cursor_position = self.char_count();
            }
            _ => {}
        }
    }

    /// Move cursor to start of current word (operates on character indices)
    fn move_cursor_start_of_word(&mut self) {
        let chars: Vec<char> = self.input.chars().collect();
        // Skip whitespace backward
        while self.cursor_position > 0 && chars[self.cursor_position - 1].is_whitespace() {
            self.cursor_position -= 1;
        }
        // Skip non-whitespace (the word) backward
        while self.cursor_position > 0 && !chars[self.cursor_position - 1].is_whitespace() {
            self.cursor_position -= 1;
        }
    }

    /// Build the text lines for the conversation.
    /// Returns owned Lines (no borrows on self) so we can modify scroll_offset afterwards.
    fn build_conversation_lines(&self) -> Vec<Line<'static>> {
        let mut text_lines = Vec::new();

        for msg in &self.messages {
            if let Some(rest) = msg.strip_prefix("You: ") {
                text_lines.push(Line::from(vec![
                    Span::styled("You: ".to_string(), Style::default().fg(Color::Green)),
                    Span::raw(rest.to_string()),
                ]));
            } else if let Some(rest) = msg.strip_prefix("Assistant: ") {
                text_lines.push(Line::from(vec![
                    Span::styled("Assistant: ".to_string(), Style::default().fg(Color::Blue)),
                    Span::raw(rest.to_string()),
                ]));
            } else {
                text_lines.push(Line::from(msg.clone()));
            }

            // Add empty line between messages
            text_lines.push(Line::from(""));
        }

        text_lines
    }

    /// Estimate the total number of rendered lines after wrapping.
    /// Each Line is at least 1 rendered line; long lines wrap to fill more.
    fn estimate_rendered_lines(lines: &[Line], wrap_width: usize) -> usize {
        if wrap_width == 0 {
            return lines.len();
        }
        lines
            .iter()
            .map(|line| {
                // Estimate the display width of this line
                let width: usize = line.spans.iter().map(|s| {
                    s.content.chars().map(|c| if c.is_ascii() { 1 } else { 2 }).sum::<usize>()
                }).sum();
                // At least 1 rendered line, even if empty
                1usize.max((width + wrap_width - 1) / wrap_width)
            })
            .sum()
    }

    /// Render the conversation history panel
    fn render_conversation(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
        let text_lines = self.build_conversation_lines();

        // Available height inside the borders
        let visible_height = area.height.saturating_sub(2) as usize;
        // Available width inside the borders for wrapping
        let wrap_width = area.width.saturating_sub(2) as usize;

        let total_rendered = Self::estimate_rendered_lines(&text_lines, wrap_width);
        let max_scroll = total_rendered.saturating_sub(visible_height);

        // Auto-scroll to bottom when follow_tail is on
        if self.follow_tail {
            self.scroll_offset = max_scroll;
        } else {
            // Clamp manual scroll to valid range
            self.scroll_offset = self.scroll_offset.min(max_scroll);
            // If user scrolled back to the bottom, re-enable follow_tail
            if self.scroll_offset >= max_scroll {
                self.follow_tail = true;
            }
        }

        let messages_paragraph = Paragraph::new(text_lines)
            .block(Block::default().borders(Borders::ALL).title("Conversation"))
            .wrap(Wrap { trim: true })
            .scroll((self.scroll_offset as u16, 0));

        f.render_widget(messages_paragraph, area);
    }

    /// Render the input area
    fn render_input(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        // Create input field with indicator
        let input_text = if self.processing {
            "Processing... Please wait."
        } else {
            &self.input[..]
        };

        let input_field = Paragraph::new(input_text)
            .block(Block::default().borders(Borders::ALL).title("Input (Press Ctrl+C to quit)"));

        f.render_widget(input_field, area);

        // Set cursor position using display width (handles CJK wide chars correctly)
        if !self.processing {
            let display_col = self.cursor_display_width();
            f.set_cursor_position((area.x + display_col + 1, area.y + 1));
        }
    }

    /// Render the header info panel (left side of the header).
    /// Shows status badge, message count, and keyboard shortcuts.
    fn render_header_info(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let status_line = if self.processing {
            Line::from(vec![
                Span::raw("  "),
                Span::styled(" PROCESSING ", Style::default().bg(Color::Yellow).fg(Color::Black)),
            ])
        } else {
            Line::from(vec![
                Span::raw("  "),
                Span::styled(" READY ", Style::default().bg(Color::Green).fg(Color::Black)),
            ])
        };

        let msg_count = self.messages.len();

        let lines = vec![
            status_line,
            Line::from(""),
            Line::from(vec![
                Span::styled("  Messages: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{}", msg_count),
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled("  Shortcuts", Style::default().fg(Color::DarkGray).add_modifier(Modifier::UNDERLINED))),
            Line::from(vec![
                Span::styled("  Enter  ", Style::default().fg(Color::Cyan)),
                Span::styled("submit", Style::default().fg(Color::DarkGray)),
            ]),
            Line::from(vec![
                Span::styled("  /      ", Style::default().fg(Color::Cyan)),
                Span::styled("commands", Style::default().fg(Color::DarkGray)),
            ]),
        ];

        let info = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" miniclaw ")
                .border_style(Style::default().fg(Color::DarkGray)),
        );

        f.render_widget(info, area);
    }

    /// Render the pet panel with animated frames.
    fn render_pet(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let state = &self.pet_state;
        let art_color = state.color();
        let frame = state.current_frame(self.anim_tick);

        let mut lines: Vec<Line> = Vec::new();

        // ASCII art lines
        for art_line in frame {
            lines.push(Line::from(
                Span::styled(*art_line, Style::default().fg(art_color)),
            ));
        }

        // Blank separator
        lines.push(Line::from(""));

        // State label (centered, bold)
        lines.push(Line::from(
            Span::styled(
                state.label(),
                Style::default()
                    .fg(art_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ));

        let pet_widget = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Pet ")
                    .border_style(Style::default().fg(art_color)),
            )
            .alignment(Alignment::Center);

        f.render_widget(pet_widget, area);
    }

    /// Draw the full UI layout.
    ///
    /// ```text
    /// ┌─ miniclaw ─────────────────┬── Pet ──────┐  ← header (fixed height)
    /// │  ■ READY                   │    /\_/\    │
    /// │  Messages: 3               │   ( o.o )   │
    /// │  ...                       │    > ^ <    │
    /// │                            │    Idle     │
    /// ├─ Conversation ─────────────┴─────────────┤  ← conversation (flexible, full width)
    /// │ ...                                      │
    /// ├─ Input ──────────────────────────────────┤  ← input (fixed height)
    /// │ > _                                      │
    /// └──────────────────────────────────────────┘
    /// ```
    fn draw_ui(&mut self, f: &mut Frame) {
        let area = f.area();

        // Vertical: [header, conversation, input]
        let rows = Layout::vertical([
            Constraint::Length(HEADER_HEIGHT),  // header (info + pet)
            Constraint::Min(4),                // conversation (full width)
            Constraint::Length(3),              // input bar
        ])
        .split(area);

        let header_area = rows[0];
        let conversation_area = rows[1];
        let input_area = rows[2];

        // Header: horizontal split [info panel | pet panel]
        let header_cols = Layout::horizontal([
            Constraint::Min(20),                    // info panel
            Constraint::Length(PET_PANEL_WIDTH),     // pet panel
        ])
        .split(header_area);

        self.render_header_info(f, header_cols[0]);
        self.render_pet(f, header_cols[1]);
        self.render_conversation(f, conversation_area);
        self.render_input(f, input_area);
    }
}

#[async_trait]
impl Ui for RatatuiUi {
    async fn run(&mut self, mut agent: Agent) -> Result<(Agent, UiExitAction)> {
        // Ensure terminal is in a clean state before initializing ratatui.
        // rustyline (which also uses crossterm internally) may have left
        // the raw-mode reference count or event reader in an inconsistent state.
        let _ = terminal::disable_raw_mode();

        // Drain any stale events left in crossterm's global event reader
        // (from rustyline, show_command_menu, or previous TUI session).
        while event::poll(std::time::Duration::from_millis(5))? {
            let _ = event::read()?;
        }

        // Setup terminal using ratatui's built-in init
        let mut terminal = ratatui::init();

        // Guard ensures ratatui::restore() is called even on early return / panic
        let _guard = TerminalGuard;

        let exit_action;

        // Main event loop
        loop {
            // Advance animation tick every cycle (~100ms)
            self.anim_tick = self.anim_tick.wrapping_add(1);

            terminal.draw(|f| self.draw_ui(f))?;

            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    // Any key press resets idle counter, boosts typing intensity
                    if !self.processing {
                        self.idle_ticks = 0;
                        self.typing_intensity = self.typing_intensity
                            .saturating_add(TYPING_BOOST_PER_KEY)
                            .min(40);
                    }

                    match key.code {
                        KeyCode::Enter => {
                            if !self.input.trim().is_empty() && !self.processing {
                                let user_input = self.input.clone();

                                // Handle slash commands
                                if user_input.starts_with('/') {
                                    match user_input.as_str() {
                                        "/quit" | "/exit" => {
                                            exit_action = UiExitAction::Quit;
                                            break;
                                        }
                                        "/ui" => {
                                            exit_action = UiExitAction::SwitchUi("terminal".to_string());
                                            break;
                                        }
                                        "/clear" => {
                                            agent.clear_history();
                                            self.messages.clear();
                                            self.messages.push("Conversation cleared.".to_string());
                                            self.scroll_offset = 0;
                                            self.follow_tail = true;
                                            self.input.clear();
                                            self.cursor_position = 0;
                                            continue;
                                        }
                                        "/help" => {
                                            self.messages.push("--- Commands ---".to_string());
                                            self.messages.push("  /help   - Show available commands".to_string());
                                            self.messages.push("  /clear  - Clear conversation history".to_string());
                                            self.messages.push("  /ui     - Switch to CLI mode".to_string());
                                            self.messages.push("  /quit   - Exit the program".to_string());
                                            self.messages.push("  Ctrl+C  - Exit the program".to_string());
                                            self.input.clear();
                                            self.cursor_position = 0;
                                            continue;
                                        }
                                        _ => {
                                            self.messages.push(format!("Unknown command: {}. Type /help for available commands.", user_input));
                                            self.input.clear();
                                            self.cursor_position = 0;
                                            continue;
                                        }
                                    }
                                }

                                // Normal message -> send to agent
                                self.messages.push(format!("You: {}", user_input));

                                // Clear input
                                self.input.clear();
                                self.cursor_position = 0;

                                // Indicate processing
                                self.processing = true;
                                self.pet_state = PetState::Thinking;
                                self.idle_ticks = 0;

                                // Auto-scroll to bottom so user can see their message
                                self.follow_tail = true;

                                // Redraw NOW so the user sees their message + "PROCESSING" status
                                // before the (potentially slow) LLM call blocks the loop.
                                terminal.draw(|f| self.draw_ui(f))?;

                                // Process with agent (this may take a while)
                                match agent.process_message(&user_input).await {
                                    Ok(response) => {
                                        self.messages.push(format!("Assistant: {}", response));
                                        self.pet_state = PetState::Happy;
                                    }
                                    Err(e) => {
                                        self.messages.push(format!("Error: {}", e));
                                        self.pet_state = PetState::Error;
                                    }
                                }

                                self.processing = false;
                                self.idle_ticks = 0;

                                // Auto-scroll to show the new response
                                self.follow_tail = true;
                            }
                        }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            exit_action = UiExitAction::Quit;
                            break;
                        }
                        KeyCode::Up => {
                            // Scroll up: stop following tail, go back in history
                            self.follow_tail = false;
                            self.scroll_offset = self.scroll_offset.saturating_sub(1);
                        }
                        KeyCode::Down => {
                            // Scroll down: move toward latest messages
                            self.scroll_offset += 1;
                            // render_conversation will clamp to max and re-enable follow_tail
                            // if we're already at the bottom
                        }
                        _ => {
                            if !self.processing {
                                self.handle_key_event(key);
                            }
                        }
                    }
                }
            } else {
                // No event received — update idle and typing counters.
                // poll timeout is 100ms, so ~300 ticks ≈ 30 seconds.
                if !self.processing {
                    self.idle_ticks += 1;
                    self.typing_intensity = self.typing_intensity.saturating_sub(TYPING_DECAY_PER_TICK);
                }
            }

            // ── Pet state machine (runs every tick) ──
            if !self.processing {
                if self.typing_intensity > TYPING_FAST_THRESHOLD {
                    self.pet_state = PetState::TypingFast;
                } else if self.typing_intensity > 0 && !self.input.is_empty() {
                    self.pet_state = PetState::Typing;
                } else if self.idle_ticks > 300 {
                    // ~30s idle → sleeping
                    self.pet_state = PetState::Sleeping;
                } else if self.pet_state == PetState::Happy && self.idle_ticks > 50 {
                    // ~5s after happy → idle
                    self.pet_state = PetState::Idle;
                } else if self.pet_state == PetState::Error && self.idle_ticks > 50 {
                    // ~5s after error → idle
                    self.pet_state = PetState::Idle;
                } else if self.pet_state == PetState::Typing
                    || self.pet_state == PetState::TypingFast
                {
                    // Typing intensity dropped to 0 → idle
                    if self.typing_intensity == 0 {
                        self.pet_state = PetState::Idle;
                    }
                }
                // else: keep current state (Idle, Happy, Error, Sleeping stay until overridden)
            }
        }

        // TerminalGuard's Drop will call ratatui::restore() automatically
        // when _guard goes out of scope (including on errors).
        // We drop it explicitly here so restore happens before the return.
        drop(_guard);

        Ok((agent, exit_action))
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