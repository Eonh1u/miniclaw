//! Modern TUI implementation using ratatui with pluggable header widgets.

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal;
use ratatui::{
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::agent::Agent;
use crate::config::AppConfig;
use crate::ui::{HeaderWidget, UiExitAction, WidgetContext};

struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

// ── PetState (public so other modules can reference it) ─────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PetState {
    Idle,
    Typing,
    TypingFast,
    Thinking,
    Happy,
    Error,
    Sleeping,
}

type ArtFrame = &'static [&'static str];

impl PetState {
    fn frames(&self) -> &[ArtFrame] {
        match self {
            PetState::Idle => &[
                &[r"   /\_/\  ", r"  ( o.o ) ", r"   > ^ <  ", r"  /|   |\ ", r"  (___)   "],
                &[r"   /\_/\  ", r"  ( o.o ) ", r"   > ^ <  ", r"  /|   |\ ", r"   (___)  "],
                &[r"   /\_/\  ", r"  ( -.- ) ", r"   > ^ <  ", r"  /|   |\ ", r"  (___)   "],
                &[r"   /\_/\  ", r"  ( o.o ) ", r"   > ^ <  ", r"  /|   |\ ", r"   (___)  "],
            ],
            PetState::Typing => &[
                &[r"   /\_/\  ", r"  ( o.o ) ", r"   >   <  ", r"   _[_]_  ", r"   click  "],
                &[r"   /\_/\  ", r"  ( o.  ) ", r"   >   <  ", r"   _[_]_  ", r"   clack  "],
                &[r"   /\_/\  ", r"  ( .o ) ", r"   >   <  ", r"   _[_]_  ", r"   click  "],
            ],
            PetState::TypingFast => &[
                &[r"   /\_/\  ", r"  ( O.O ) ", r"   >   <  ", r"  _[===]_ ", r"  *CLACK* "],
                &[r"  ~/\_/\~ ", r"  ( O.O ) ", r"   >   <  ", r"  _[===]_ ", r"  *CLICK* "],
                &[r"   /\_/\  ", r"  ( O.O ) ", r"   >   <  ", r"  _[===]_ ", r"  *CLACK* "],
                &[r" ~/\_/\ ~ ", r"  ( O.O ) ", r"   >   <  ", r"  _[===]_ ", r"  *CLICK* "],
            ],
            PetState::Thinking => &[
                &[r"   /\_/\  ", r"  ( o.O ) ", r"   > ~ <  ", r"  /|   |\ ", r"    ...   "],
                &[r"   /\_/\  ", r"  ( O.o ) ", r"   > ~ <  ", r"  /|   |\ ", r"   ...    "],
                &[r"   /\_/\  ", r"  ( o.o ) ", r"   > ? <  ", r"  /|   |\ ", r"     ..   "],
                &[r"   /\_/\  ", r"  ( O.O ) ", r"   > ~ <  ", r"  /|   |\ ", r"  . . .   "],
            ],
            PetState::Happy => &[
                &[r"   /\_/\  ", r"  ( ^.^ ) ", r"   > v <  ", r"  /|   |\ ", r"  * ~ * ~ "],
                &[r"  ~/\_/\  ", r"  ( ^o^ ) ", r"   > v <  ", r"  /|   |\ ", r"  ~ * ~ * "],
                &[r"   /\_/\~ ", r"  ( ^.^ ) ", r"   > v <  ", r"  /|   |\ ", r"  * * ~ ~ "],
                &[r"  ~/\_/\  ", r"  ( ^o^ ) ", r"   > v <  ", r"  /|   |\ ", r"  ~ ~ * * "],
            ],
            PetState::Error => &[
                &[r"   /\_/\  ", r"  ( T.T ) ", r"   > _ <  ", r"  /|   |\ ", r"   ...    "],
                &[r"   /\_/\  ", r"  ( ;.; ) ", r"   > _ <  ", r"  /|   |\ ", r"    ...   "],
            ],
            PetState::Sleeping => &[
                &[r"   /\_/\  ", r"  ( -.- ) ", r"   > z <  ", r"  /|   |\ ", r"      z   "],
                &[r"   /\_/\  ", r"  ( -.- ) ", r"   > z <  ", r"  /|   |\ ", r"     zZ   "],
                &[r"   /\_/\  ", r"  ( -.- ) ", r"   > z <  ", r"  /|   |\ ", r"    zZz   "],
                &[r"   /\_/\  ", r"  ( -.- ) ", r"   > z <  ", r"  /|   |\ ", r"   zZzZ   "],
            ],
        }
    }

    fn ticks_per_frame(&self) -> u32 {
        match self {
            PetState::Idle       => 8,
            PetState::Typing     => 4,
            PetState::TypingFast => 2,
            PetState::Thinking   => 5,
            PetState::Happy      => 3,
            PetState::Error      => 6,
            PetState::Sleeping   => 10,
        }
    }

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

    fn current_frame(&self, tick: u32) -> ArtFrame {
        let frames = self.frames();
        let idx = (tick / self.ticks_per_frame()) as usize % frames.len();
        frames[idx]
    }
}

// ── Built-in Header Widgets ─────────────────────────────────

/// Stats widget: shows token counts and usage days.
pub struct StatsWidget;

impl HeaderWidget for StatsWidget {
    fn id(&self) -> &str { "stats" }
    fn preferred_width(&self) -> Option<u16> { None } // fill remaining

    fn render(&self, f: &mut Frame, area: ratatui::layout::Rect, ctx: &WidgetContext) {
        let stats = &ctx.agent.stats;

        let status_line = if ctx.processing {
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

        let usage_days = ctx.first_use_date.map_or(1i64, |first| {
            let today = chrono::Local::now().date_naive();
            (today - first).num_days().max(0) + 1
        });

        let lines = vec![
            status_line,
            Line::from(""),
            Line::from(vec![
                Span::styled("  In: ", Style::default().fg(Color::DarkGray)),
                Span::styled(format_token_count(stats.total_input_tokens), Style::default().fg(Color::Cyan)),
                Span::styled("  Out: ", Style::default().fg(Color::DarkGray)),
                Span::styled(format_token_count(stats.total_output_tokens), Style::default().fg(Color::Magenta)),
            ]),
            Line::from(vec![
                Span::styled("  Requests: ", Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{}", stats.request_count), Style::default().fg(Color::White)),
                Span::styled("  Day: ", Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{}", usage_days), Style::default().fg(Color::Yellow)),
            ]),
            Line::from(""),
            Line::from(Span::styled("  Shortcuts", Style::default().fg(Color::DarkGray).add_modifier(Modifier::UNDERLINED))),
            Line::from(vec![
                Span::styled("  Enter ", Style::default().fg(Color::Cyan)),
                Span::styled("submit  ", Style::default().fg(Color::DarkGray)),
                Span::styled("/help ", Style::default().fg(Color::Cyan)),
                Span::styled("cmds", Style::default().fg(Color::DarkGray)),
            ]),
        ];

        let widget = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" miniclaw ")
                .border_style(Style::default().fg(Color::DarkGray)),
        );
        f.render_widget(widget, area);
    }
}

/// Pet animation widget.
pub struct PetWidget;

impl HeaderWidget for PetWidget {
    fn id(&self) -> &str { "pet" }
    fn preferred_width(&self) -> Option<u16> { Some(20) }

    fn render(&self, f: &mut Frame, area: ratatui::layout::Rect, ctx: &WidgetContext) {
        let state = &ctx.pet_state;
        let art_color = state.color();
        let frame = state.current_frame(ctx.anim_tick);

        let mut lines: Vec<Line> = Vec::new();
        for art_line in frame {
            lines.push(Line::from(Span::styled(*art_line, Style::default().fg(art_color))));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            state.label(),
            Style::default().fg(art_color).add_modifier(Modifier::BOLD),
        )));

        let widget = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Pet ")
                    .border_style(Style::default().fg(art_color)),
            )
            .alignment(Alignment::Center);
        f.render_widget(widget, area);
    }
}

fn format_token_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{}", n)
    }
}

// ── Persistence helpers ─────────────────────────────────────

fn usage_data_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(".miniclaw").join("usage.json"))
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct UsageData {
    first_use_date: Option<String>,
}

fn load_first_use_date() -> Option<chrono::NaiveDate> {
    let path = usage_data_path()?;
    let content = std::fs::read_to_string(&path).ok()?;
    let data: UsageData = serde_json::from_str(&content).ok()?;
    data.first_use_date.and_then(|s| s.parse().ok())
}

fn save_first_use_date(date: chrono::NaiveDate) {
    if let Some(path) = usage_data_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let data = UsageData {
            first_use_date: Some(date.to_string()),
        };
        if let Ok(json) = serde_json::to_string_pretty(&data) {
            let _ = std::fs::write(&path, json);
        }
    }
}

fn ensure_first_use_date() -> Option<chrono::NaiveDate> {
    if let Some(date) = load_first_use_date() {
        return Some(date);
    }
    let today = chrono::Local::now().date_naive();
    save_first_use_date(today);
    Some(today)
}

// ── TUI State ───────────────────────────────────────────────

const HEADER_HEIGHT: u16 = 10;
const TYPING_FAST_THRESHOLD: u32 = 15;
const TYPING_DECAY_PER_TICK: u32 = 1;
const TYPING_BOOST_PER_KEY: u32 = 4;

pub struct RatatuiUi {
    input: String,
    cursor_position: usize,
    messages: Vec<String>,
    scroll_offset: usize,
    follow_tail: bool,
    processing: bool,
    pet_state: PetState,
    anim_tick: u32,
    idle_ticks: u32,
    typing_intensity: u32,
    header_widgets: Vec<Box<dyn HeaderWidget>>,
    first_use_date: Option<chrono::NaiveDate>,
}

impl RatatuiUi {
    pub fn new(config: &AppConfig) -> Self {
        let mut header_widgets: Vec<Box<dyn HeaderWidget>> = Vec::new();
        if config.ui.show_stats {
            header_widgets.push(Box::new(StatsWidget));
        }
        if config.ui.show_pet {
            header_widgets.push(Box::new(PetWidget));
        }

        Self {
            input: String::new(),
            cursor_position: 0,
            messages: vec!["Welcome to miniclaw! Type your message or /help for commands.".to_string()],
            scroll_offset: 0,
            follow_tail: true,
            processing: false,
            pet_state: PetState::Idle,
            anim_tick: 0,
            idle_ticks: 0,
            typing_intensity: 0,
            header_widgets,
            first_use_date: ensure_first_use_date(),
        }
    }

    /// Toggle a widget by id. Returns true if the widget is now visible.
    fn toggle_widget(&mut self, id: &str) -> bool {
        if let Some(pos) = self.header_widgets.iter().position(|w| w.id() == id) {
            self.header_widgets.remove(pos);
            false
        } else {
            match id {
                "stats" => self.header_widgets.insert(0, Box::new(StatsWidget)),
                "pet" => self.header_widgets.push(Box::new(PetWidget)),
                _ => return false,
            }
            true
        }
    }

    // --- UTF-8 safe cursor helpers ---

    fn byte_index(&self) -> usize {
        self.input.char_indices().nth(self.cursor_position).map_or(self.input.len(), |(i, _)| i)
    }

    fn char_count(&self) -> usize {
        self.input.chars().count()
    }

    fn cursor_display_width(&self) -> u16 {
        self.input.chars().take(self.cursor_position)
            .map(|c| if c.is_ascii() { 1u16 } else { 2u16 })
            .sum()
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(c) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    match c {
                        'u' => { self.input.clear(); self.cursor_position = 0; }
                        'k' => { let b = self.byte_index(); self.input.drain(b..); }
                        'w' => {
                            let end = self.byte_index();
                            self.move_cursor_start_of_word();
                            let start = self.byte_index();
                            self.input.drain(start..end);
                        }
                        _ => {}
                    }
                } else {
                    let b = self.byte_index();
                    self.input.insert(b, c);
                    self.cursor_position += 1;
                }
            }
            KeyCode::Backspace => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                    let b = self.byte_index();
                    self.input.remove(b);
                }
            }
            KeyCode::Delete => {
                if self.cursor_position < self.char_count() {
                    let b = self.byte_index();
                    self.input.remove(b);
                }
            }
            KeyCode::Left  => { if self.cursor_position > 0 { self.cursor_position -= 1; } }
            KeyCode::Right => { if self.cursor_position < self.char_count() { self.cursor_position += 1; } }
            KeyCode::Home  => { self.cursor_position = 0; }
            KeyCode::End   => { self.cursor_position = self.char_count(); }
            _ => {}
        }
    }

    fn move_cursor_start_of_word(&mut self) {
        let chars: Vec<char> = self.input.chars().collect();
        while self.cursor_position > 0 && chars[self.cursor_position - 1].is_whitespace() {
            self.cursor_position -= 1;
        }
        while self.cursor_position > 0 && !chars[self.cursor_position - 1].is_whitespace() {
            self.cursor_position -= 1;
        }
    }

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
            text_lines.push(Line::from(""));
        }
        text_lines
    }

    fn estimate_rendered_lines(lines: &[Line], wrap_width: usize) -> usize {
        if wrap_width == 0 { return lines.len(); }
        lines.iter().map(|line| {
            let width: usize = line.spans.iter()
                .map(|s| s.content.chars().map(|c| if c.is_ascii() { 1 } else { 2 }).sum::<usize>())
                .sum();
            1usize.max((width + wrap_width - 1) / wrap_width)
        }).sum()
    }

    fn render_conversation(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
        let text_lines = self.build_conversation_lines();
        let visible_height = area.height.saturating_sub(2) as usize;
        let wrap_width = area.width.saturating_sub(2) as usize;
        let total_rendered = Self::estimate_rendered_lines(&text_lines, wrap_width);
        let max_scroll = total_rendered.saturating_sub(visible_height);

        if self.follow_tail {
            self.scroll_offset = max_scroll;
        } else {
            self.scroll_offset = self.scroll_offset.min(max_scroll);
            if self.scroll_offset >= max_scroll { self.follow_tail = true; }
        }

        let p = Paragraph::new(text_lines)
            .block(Block::default().borders(Borders::ALL).title("Conversation"))
            .wrap(Wrap { trim: true })
            .scroll((self.scroll_offset as u16, 0));
        f.render_widget(p, area);
    }

    fn render_input(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let input_text = if self.processing { "Processing... Please wait." } else { &self.input[..] };
        let p = Paragraph::new(input_text)
            .block(Block::default().borders(Borders::ALL).title("Input (Ctrl+C quit)"));
        f.render_widget(p, area);
        if !self.processing {
            let col = self.cursor_display_width();
            f.set_cursor_position((area.x + col + 1, area.y + 1));
        }
    }

    fn render_header(&self, f: &mut Frame, area: ratatui::layout::Rect, agent: &Agent) {
        if self.header_widgets.is_empty() {
            return;
        }

        let ctx = WidgetContext {
            agent,
            messages: &self.messages,
            processing: self.processing,
            anim_tick: self.anim_tick,
            pet_state: self.pet_state,
            idle_ticks: self.idle_ticks,
            typing_intensity: self.typing_intensity,
            first_use_date: self.first_use_date,
        };

        // Build layout constraints from widgets
        let constraints: Vec<Constraint> = self.header_widgets.iter().map(|w| {
            match w.preferred_width() {
                Some(width) => Constraint::Length(width),
                None => Constraint::Min(20),
            }
        }).collect();

        let cols = Layout::horizontal(constraints).split(area);

        for (i, widget) in self.header_widgets.iter().enumerate() {
            if i < cols.len() {
                widget.render(f, cols[i], &ctx);
            }
        }
    }

    fn draw_ui(&mut self, f: &mut Frame, agent: &Agent) {
        let area = f.area();

        let header_h = if self.header_widgets.is_empty() { 0 } else { HEADER_HEIGHT };
        let rows = Layout::vertical([
            Constraint::Length(header_h),
            Constraint::Min(4),
            Constraint::Length(3),
        ]).split(area);

        if header_h > 0 {
            self.render_header(f, rows[0], agent);
        }
        self.render_conversation(f, rows[1]);
        self.render_input(f, rows[2]);
    }

    /// Handle a slash command. Returns Some(action) to break the loop, or None.
    fn handle_command(&mut self, cmd: &str, agent: &mut Agent) -> Option<UiExitAction> {
        match cmd {
            "/quit" | "/exit" => return Some(UiExitAction::Quit),
            "/clear" => {
                agent.clear_history();
                self.messages.clear();
                self.messages.push("Conversation cleared.".to_string());
                self.scroll_offset = 0;
                self.follow_tail = true;
            }
            "/stats" => {
                let visible = self.toggle_widget("stats");
                self.messages.push(format!("[Stats panel {}]", if visible { "enabled" } else { "disabled" }));
            }
            "/pet" => {
                let visible = self.toggle_widget("pet");
                self.messages.push(format!("[Pet panel {}]", if visible { "enabled" } else { "disabled" }));
            }
            "/help" => {
                self.messages.push("--- Commands ---".to_string());
                self.messages.push("  /help   - Show available commands".to_string());
                self.messages.push("  /clear  - Clear conversation history".to_string());
                self.messages.push("  /stats  - Toggle stats panel".to_string());
                self.messages.push("  /pet    - Toggle pet panel".to_string());
                self.messages.push("  /quit   - Exit the program".to_string());
                self.messages.push("  Ctrl+C  - Exit the program".to_string());
            }
            other => {
                self.messages.push(format!("Unknown command: {}. Type /help for commands.", other));
            }
        }
        None
    }

    pub async fn run(mut self, mut agent: Agent) -> Result<(Agent, UiExitAction)> {
        let _ = terminal::disable_raw_mode();
        while event::poll(std::time::Duration::from_millis(5))? {
            let _ = event::read()?;
        }

        let mut terminal = ratatui::init();
        let _guard = TerminalGuard;
        let exit_action;

        loop {
            self.anim_tick = self.anim_tick.wrapping_add(1);
            terminal.draw(|f| self.draw_ui(f, &agent))?;

            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if !self.processing {
                        self.idle_ticks = 0;
                        self.typing_intensity = self.typing_intensity
                            .saturating_add(TYPING_BOOST_PER_KEY).min(40);
                    }

                    match key.code {
                        KeyCode::Enter => {
                            if !self.input.trim().is_empty() && !self.processing {
                                let user_input = self.input.clone();
                                self.input.clear();
                                self.cursor_position = 0;

                                if user_input.starts_with('/') {
                                    if let Some(action) = self.handle_command(&user_input, &mut agent) {
                                        exit_action = action;
                                        break;
                                    }
                                    continue;
                                }

                                self.messages.push(format!("You: {}", user_input));
                                self.processing = true;
                                self.pet_state = PetState::Thinking;
                                self.idle_ticks = 0;
                                self.follow_tail = true;
                                terminal.draw(|f| self.draw_ui(f, &agent))?;

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
                                self.follow_tail = true;
                            }
                        }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            exit_action = UiExitAction::Quit;
                            break;
                        }
                        KeyCode::Up => {
                            self.follow_tail = false;
                            self.scroll_offset = self.scroll_offset.saturating_sub(1);
                        }
                        KeyCode::Down => {
                            self.scroll_offset += 1;
                        }
                        _ => {
                            if !self.processing {
                                self.handle_key_event(key);
                            }
                        }
                    }
                }
            } else {
                if !self.processing {
                    self.idle_ticks += 1;
                    self.typing_intensity = self.typing_intensity.saturating_sub(TYPING_DECAY_PER_TICK);
                }
            }

            // Pet state machine
            if !self.processing {
                if self.typing_intensity > TYPING_FAST_THRESHOLD {
                    self.pet_state = PetState::TypingFast;
                } else if self.typing_intensity > 0 && !self.input.is_empty() {
                    self.pet_state = PetState::Typing;
                } else if self.idle_ticks > 300 {
                    self.pet_state = PetState::Sleeping;
                } else if self.pet_state == PetState::Happy && self.idle_ticks > 50 {
                    self.pet_state = PetState::Idle;
                } else if self.pet_state == PetState::Error && self.idle_ticks > 50 {
                    self.pet_state = PetState::Idle;
                } else if (self.pet_state == PetState::Typing || self.pet_state == PetState::TypingFast)
                    && self.typing_intensity == 0
                {
                    self.pet_state = PetState::Idle;
                }
            }
        }

        drop(_guard);
        Ok((agent, exit_action))
    }
}
