//! Modern TUI implementation using ratatui with pluggable header widgets.

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal;
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::agent::{Agent, AgentEvent};
use crate::config::AppConfig;
use crate::ui::{HeaderWidget, UiExitAction, WidgetContext};

// ── Slash Command Definitions ───────────────────────────────

struct SlashCommand {
    name: &'static str,
    description: &'static str,
}

const SLASH_COMMANDS: &[SlashCommand] = &[
    SlashCommand { name: "/help",  description: "Show available commands" },
    SlashCommand { name: "/clear", description: "Clear conversation history" },
    SlashCommand { name: "/stats", description: "Toggle stats panel" },
    SlashCommand { name: "/pet",   description: "Toggle pet panel" },
    SlashCommand { name: "/quit",  description: "Exit the program" },
    SlashCommand { name: "/exit",  description: "Exit the program" },
];

/// Check if input looks like a slash command (e.g. "/help", "/clear"),
/// as opposed to a file path like "/root/code/..." or "/tmp/file.txt".
fn is_slash_command(input: &str) -> bool {
    let input = input.trim();
    if !input.starts_with('/') {
        return false;
    }
    let after_slash = &input[1..];
    !after_slash.is_empty() && after_slash.chars().all(|c| c.is_ascii_lowercase())
}

/// Autocomplete popup state for slash commands.
struct SlashAutocomplete {
    visible: bool,
    selected: usize,
    filtered: Vec<usize>,
}

impl SlashAutocomplete {
    fn new() -> Self {
        Self {
            visible: false,
            selected: 0,
            filtered: Vec::new(),
        }
    }

    fn update_filter(&mut self, input: &str) {
        if !is_slash_command(input) && input != "/" {
            self.visible = false;
            self.filtered.clear();
            self.selected = 0;
            return;
        }

        let query = input.to_lowercase();
        self.filtered = SLASH_COMMANDS
            .iter()
            .enumerate()
            .filter(|(_, cmd)| cmd.name.starts_with(&query))
            .map(|(i, _)| i)
            .collect();

        self.visible = !self.filtered.is_empty();
        if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len().saturating_sub(1);
        }
    }

    fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        } else {
            self.selected = self.filtered.len().saturating_sub(1);
        }
    }

    fn move_down(&mut self) {
        if self.selected + 1 < self.filtered.len() {
            self.selected += 1;
        } else {
            self.selected = 0;
        }
    }

    fn selected_command(&self) -> Option<&'static str> {
        self.filtered
            .get(self.selected)
            .map(|&i| SLASH_COMMANDS[i].name)
    }

    fn dismiss(&mut self) {
        self.visible = false;
        self.filtered.clear();
        self.selected = 0;
    }
}

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
        let stats = ctx.stats;

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
    autocomplete: SlashAutocomplete,
    cached_stats: crate::agent::SessionStats,
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
            autocomplete: SlashAutocomplete::new(),
            cached_stats: crate::agent::SessionStats::default(),
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
        self.autocomplete.update_filter(&self.input);
    }

    fn apply_autocomplete_selection(&mut self) {
        if let Some(cmd) = self.autocomplete.selected_command() {
            self.input = cmd.to_string();
            self.cursor_position = self.input.chars().count();
            self.autocomplete.dismiss();
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
                text_lines.push(Line::from(""));
            } else if let Some(rest) = msg.strip_prefix("Assistant: ") {
                text_lines.push(Line::from(Span::styled(
                    "Assistant:".to_string(),
                    Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
                )));
                let md_lines = crate::ui::markdown::markdown_to_lines(rest);
                text_lines.extend(md_lines);
            } else {
                text_lines.push(Line::from(msg.clone()));
                text_lines.push(Line::from(""));
            }
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

    fn render_input(&self, f: &mut Frame, area: Rect) {
        let input_text = if self.processing { "Processing... Please wait." } else { &self.input[..] };
        let p = Paragraph::new(input_text)
            .block(Block::default().borders(Borders::ALL).title("Input (Ctrl+C quit)"));
        f.render_widget(p, area);
        if !self.processing {
            let col = self.cursor_display_width();
            f.set_cursor_position((area.x + col + 1, area.y + 1));
        }
    }

    fn render_autocomplete(&self, f: &mut Frame, input_area: Rect) {
        if !self.autocomplete.visible || self.processing {
            return;
        }

        let item_count = self.autocomplete.filtered.len() as u16;
        let popup_height = item_count + 2; // +2 for borders
        let popup_width = 40u16.min(input_area.width);

        let popup_area = Rect {
            x: input_area.x,
            y: input_area.y.saturating_sub(popup_height),
            width: popup_width,
            height: popup_height,
        };

        f.render_widget(Clear, popup_area);

        let lines: Vec<Line> = self
            .autocomplete
            .filtered
            .iter()
            .enumerate()
            .map(|(i, &cmd_idx)| {
                let cmd = &SLASH_COMMANDS[cmd_idx];
                let is_selected = i == self.autocomplete.selected;
                let (bg, fg_name, fg_desc) = if is_selected {
                    (Color::Cyan, Color::Black, Color::DarkGray)
                } else {
                    (Color::Reset, Color::Cyan, Color::DarkGray)
                };
                Line::from(vec![
                    Span::styled(
                        format!(" {:<8}", cmd.name),
                        Style::default().fg(fg_name).bg(bg).add_modifier(if is_selected { Modifier::BOLD } else { Modifier::empty() }),
                    ),
                    Span::styled(
                        format!(" {}", cmd.description),
                        Style::default().fg(fg_desc).bg(bg),
                    ),
                ])
            })
            .collect();

        let popup = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(" Commands ")
                .title_style(Style::default().fg(Color::Cyan)),
        );
        f.render_widget(popup, popup_area);
    }

    fn render_header(&self, f: &mut Frame, area: ratatui::layout::Rect, stats: &crate::agent::SessionStats) {
        if self.header_widgets.is_empty() {
            return;
        }

        let ctx = WidgetContext {
            stats,
            messages: &self.messages,
            processing: self.processing,
            anim_tick: self.anim_tick,
            pet_state: self.pet_state,
            idle_ticks: self.idle_ticks,
            typing_intensity: self.typing_intensity,
            first_use_date: self.first_use_date,
        };

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

    fn draw_ui(&mut self, f: &mut Frame, stats: &crate::agent::SessionStats) {
        let area = f.area();

        let header_h = if self.header_widgets.is_empty() { 0 } else { HEADER_HEIGHT };
        let rows = Layout::vertical([
            Constraint::Length(header_h),
            Constraint::Min(4),
            Constraint::Length(3),
        ]).split(area);

        if header_h > 0 {
            self.render_header(f, rows[0], stats);
        }
        self.render_conversation(f, rows[1]);
        self.render_input(f, rows[2]);
        self.render_autocomplete(f, rows[2]);
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

    fn handle_agent_event(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::LlmText(text) => {
                self.messages.push(format!(
                    "  \u{1f4ad} {}",
                    text.lines().next().unwrap_or("").chars().take(80).collect::<String>()
                ));
                self.follow_tail = true;
            }
            AgentEvent::ToolStart { name } => {
                self.messages.push(format!("  \u{26a1} 调用 {} ...", name));
                self.follow_tail = true;
            }
            AgentEvent::ToolEnd { name, success } => {
                let icon = if success { "\u{2713}" } else { "\u{2717}" };
                let status = if success { "完成" } else { "失败" };
                self.messages.push(format!("  {} {} {}", icon, name, status));
                self.follow_tail = true;
            }
            AgentEvent::Done(response) => {
                self.messages.push(format!("Assistant: {}", response));
                self.pet_state = PetState::Happy;
                self.processing = false;
                self.idle_ticks = 0;
                self.follow_tail = true;
            }
            AgentEvent::Error(e) => {
                self.messages.push(format!("Error: {}", e));
                self.pet_state = PetState::Error;
                self.processing = false;
                self.idle_ticks = 0;
                self.follow_tail = true;
            }
        }
    }

    pub async fn run(mut self, agent: Agent) -> Result<(Agent, UiExitAction)> {
        let _ = terminal::disable_raw_mode();
        while event::poll(std::time::Duration::from_millis(5))? {
            let _ = event::read()?;
        }

        let mut terminal = ratatui::init();
        let _guard = TerminalGuard;
        let exit_action;

        self.cached_stats = agent.stats.clone();

        let mut event_rx: Option<tokio::sync::mpsc::UnboundedReceiver<AgentEvent>> = None;
        let mut agent_handle: Option<tokio::task::JoinHandle<Result<Agent>>> = None;
        let mut agent_opt: Option<Agent> = Some(agent);

        loop {
            self.anim_tick = self.anim_tick.wrapping_add(1);
            let stats_snapshot = self.cached_stats.clone();
            terminal.draw(|f| self.draw_ui(f, &stats_snapshot))?;

            if let Some(rx) = &mut event_rx {
                while let Ok(evt) = rx.try_recv() {
                    let is_terminal = matches!(evt, AgentEvent::Done(_) | AgentEvent::Error(_));
                    self.handle_agent_event(evt);
                    if is_terminal {
                        if let Some(handle) = agent_handle.take() {
                            match handle.await {
                                Ok(Ok(returned_agent)) => {
                                    self.cached_stats = returned_agent.stats.clone();
                                    agent_opt = Some(returned_agent);
                                }
                                Ok(Err(e)) => {
                                    self.messages.push(format!("Error: {}", e));
                                    self.pet_state = PetState::Error;
                                    self.processing = false;
                                }
                                Err(e) => {
                                    self.messages.push(format!("Error: task panicked: {}", e));
                                    self.pet_state = PetState::Error;
                                    self.processing = false;
                                }
                            }
                        }
                        event_rx = None;
                        break;
                    }
                }
            }

            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if !self.processing {
                        self.idle_ticks = 0;
                        self.typing_intensity = self.typing_intensity
                            .saturating_add(TYPING_BOOST_PER_KEY).min(40);
                    }

                    match key.code {
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            exit_action = UiExitAction::Quit;
                            break;
                        }
                        KeyCode::Esc if self.autocomplete.visible => {
                            self.autocomplete.dismiss();
                        }
                        KeyCode::Up if self.autocomplete.visible => {
                            self.autocomplete.move_up();
                        }
                        KeyCode::Down if self.autocomplete.visible => {
                            self.autocomplete.move_down();
                        }
                        KeyCode::Tab if self.autocomplete.visible => {
                            self.apply_autocomplete_selection();
                        }
                        KeyCode::Enter => {
                            if self.autocomplete.visible {
                                if let Some(ref mut agent) = agent_opt {
                                    self.apply_autocomplete_selection();
                                    let user_input = self.input.clone();
                                    self.input.clear();
                                    self.cursor_position = 0;
                                    self.autocomplete.dismiss();
                                    if let Some(action) = self.handle_command(&user_input, agent) {
                                        exit_action = action;
                                        break;
                                    }
                                }
                                continue;
                            }

                            if !self.input.trim().is_empty() && !self.processing {
                                let user_input = self.input.clone();
                                self.input.clear();
                                self.cursor_position = 0;
                                self.autocomplete.dismiss();

                                if is_slash_command(&user_input) {
                                    if let Some(ref mut agent) = agent_opt {
                                        if let Some(action) = self.handle_command(&user_input, agent) {
                                            exit_action = action;
                                            break;
                                        }
                                    }
                                    continue;
                                }

                                self.messages.push(format!("You: {}", user_input));
                                self.processing = true;
                                self.pet_state = PetState::Thinking;
                                self.idle_ticks = 0;
                                self.follow_tail = true;

                                if let Some(mut moved_agent) = agent_opt.take() {
                                    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                                    event_rx = Some(rx);
                                    let input_clone = user_input.clone();
                                    agent_handle = Some(tokio::spawn(async move {
                                        let result = moved_agent
                                            .process_message(&input_clone, Some(tx))
                                            .await;
                                        result.map(|_| moved_agent)
                                    }));
                                }
                            }
                        }
                        KeyCode::Up if !self.processing => {
                            self.follow_tail = false;
                            self.scroll_offset = self.scroll_offset.saturating_sub(1);
                        }
                        KeyCode::Down if !self.processing => {
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
        let agent = agent_opt.unwrap_or_else(|| {
            panic!("Agent was not returned from background task");
        });
        Ok((agent, exit_action))
    }
}
