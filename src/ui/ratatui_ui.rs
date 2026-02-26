//! Modern TUI implementation using ratatui with pluggable header widgets
//! and multi-session tab support.

use std::collections::VecDeque;
use std::path::PathBuf;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};
use crossterm::terminal;
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::agent::{Agent, AgentEvent, SessionStats};
use crate::config::AppConfig;
use crate::session::{self, SessionData, SessionStatsData};
use crate::ui::{HeaderWidget, UiExitAction, WidgetContext};

// ── Slash Command Definitions ───────────────────────────────

struct SlashCommand {
    name: &'static str,
    description: &'static str,
}

const SLASH_COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        name: "/help",
        description: "Show available commands",
    },
    SlashCommand {
        name: "/clear",
        description: "Clear conversation history",
    },
    SlashCommand {
        name: "/new",
        description: "Create new session tab",
    },
    SlashCommand {
        name: "/close",
        description: "Close current session tab",
    },
    SlashCommand {
        name: "/rename",
        description: "Rename current session (/rename <name>)",
    },
    SlashCommand {
        name: "/sessions",
        description: "List saved sessions",
    },
    SlashCommand {
        name: "/save",
        description: "Save current session (/save [name])",
    },
    SlashCommand {
        name: "/load",
        description: "Load saved session (/load <id>)",
    },
    SlashCommand {
        name: "/export",
        description: "Export session to file (/export <path>)",
    },
    SlashCommand {
        name: "/import",
        description: "Import session from file (/import <path>)",
    },
    SlashCommand {
        name: "/stats",
        description: "Toggle stats panel",
    },
    SlashCommand {
        name: "/pet",
        description: "Toggle pet panel",
    },
    SlashCommand {
        name: "/quit",
        description: "Exit the program",
    },
    SlashCommand {
        name: "/exit",
        description: "Exit the program",
    },
];

fn is_slash_command(input: &str) -> bool {
    let input = input.trim();
    if !input.starts_with('/') {
        return false;
    }
    let after_slash = &input[1..];
    if after_slash.is_empty() {
        return false;
    }
    let cmd_part = after_slash.split_whitespace().next().unwrap_or("");
    !cmd_part.is_empty() && cmd_part.chars().all(|c| c.is_ascii_lowercase())
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
        let cmd_part = input.split_whitespace().next().unwrap_or(input);
        if !is_slash_command(cmd_part) && cmd_part != "/" {
            self.visible = false;
            self.filtered.clear();
            self.selected = 0;
            return;
        }
        if input.contains(' ') {
            self.visible = false;
            self.filtered.clear();
            self.selected = 0;
            return;
        }

        let query = cmd_part.to_lowercase();
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
        let _ = crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture);
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
                &[
                    r"   /\_/\  ",
                    r"  ( o.o ) ",
                    r"   > ^ <  ",
                    r"  /|   |\ ",
                    r"  (___)   ",
                ],
                &[
                    r"   /\_/\  ",
                    r"  ( o.o ) ",
                    r"   > ^ <  ",
                    r"  /|   |\ ",
                    r"   (___)  ",
                ],
                &[
                    r"   /\_/\  ",
                    r"  ( -.- ) ",
                    r"   > ^ <  ",
                    r"  /|   |\ ",
                    r"  (___)   ",
                ],
                &[
                    r"   /\_/\  ",
                    r"  ( o.o ) ",
                    r"   > ^ <  ",
                    r"  /|   |\ ",
                    r"   (___)  ",
                ],
            ],
            PetState::Typing => &[
                &[
                    r"   /\_/\  ",
                    r"  ( o.o ) ",
                    r"   >   <  ",
                    r"   _[_]_  ",
                    r"   click  ",
                ],
                &[
                    r"   /\_/\  ",
                    r"  ( o.  ) ",
                    r"   >   <  ",
                    r"   _[_]_  ",
                    r"   clack  ",
                ],
                &[
                    r"   /\_/\  ",
                    r"  ( .o ) ",
                    r"   >   <  ",
                    r"   _[_]_  ",
                    r"   click  ",
                ],
            ],
            PetState::TypingFast => &[
                &[
                    r"   /\_/\  ",
                    r"  ( O.O ) ",
                    r"   >   <  ",
                    r"  _[===]_ ",
                    r"  *CLACK* ",
                ],
                &[
                    r"  ~/\_/\~ ",
                    r"  ( O.O ) ",
                    r"   >   <  ",
                    r"  _[===]_ ",
                    r"  *CLICK* ",
                ],
                &[
                    r"   /\_/\  ",
                    r"  ( O.O ) ",
                    r"   >   <  ",
                    r"  _[===]_ ",
                    r"  *CLACK* ",
                ],
                &[
                    r" ~/\_/\ ~ ",
                    r"  ( O.O ) ",
                    r"   >   <  ",
                    r"  _[===]_ ",
                    r"  *CLICK* ",
                ],
            ],
            PetState::Thinking => &[
                &[
                    r"   /\_/\  ",
                    r"  ( o.O ) ",
                    r"   > ~ <  ",
                    r"  /|   |\ ",
                    r"    ...   ",
                ],
                &[
                    r"   /\_/\  ",
                    r"  ( O.o ) ",
                    r"   > ~ <  ",
                    r"  /|   |\ ",
                    r"   ...    ",
                ],
                &[
                    r"   /\_/\  ",
                    r"  ( o.o ) ",
                    r"   > ? <  ",
                    r"  /|   |\ ",
                    r"     ..   ",
                ],
                &[
                    r"   /\_/\  ",
                    r"  ( O.O ) ",
                    r"   > ~ <  ",
                    r"  /|   |\ ",
                    r"  . . .   ",
                ],
            ],
            PetState::Happy => &[
                &[
                    r"   /\_/\  ",
                    r"  ( ^.^ ) ",
                    r"   > v <  ",
                    r"  /|   |\ ",
                    r"  * ~ * ~ ",
                ],
                &[
                    r"  ~/\_/\  ",
                    r"  ( ^o^ ) ",
                    r"   > v <  ",
                    r"  /|   |\ ",
                    r"  ~ * ~ * ",
                ],
                &[
                    r"   /\_/\~ ",
                    r"  ( ^.^ ) ",
                    r"   > v <  ",
                    r"  /|   |\ ",
                    r"  * * ~ ~ ",
                ],
                &[
                    r"  ~/\_/\  ",
                    r"  ( ^o^ ) ",
                    r"   > v <  ",
                    r"  /|   |\ ",
                    r"  ~ ~ * * ",
                ],
            ],
            PetState::Error => &[
                &[
                    r"   /\_/\  ",
                    r"  ( T.T ) ",
                    r"   > _ <  ",
                    r"  /|   |\ ",
                    r"   ...    ",
                ],
                &[
                    r"   /\_/\  ",
                    r"  ( ;.; ) ",
                    r"   > _ <  ",
                    r"  /|   |\ ",
                    r"    ...   ",
                ],
            ],
            PetState::Sleeping => &[
                &[
                    r"   /\_/\  ",
                    r"  ( -.- ) ",
                    r"   > z <  ",
                    r"  /|   |\ ",
                    r"      z   ",
                ],
                &[
                    r"   /\_/\  ",
                    r"  ( -.- ) ",
                    r"   > z <  ",
                    r"  /|   |\ ",
                    r"     zZ   ",
                ],
                &[
                    r"   /\_/\  ",
                    r"  ( -.- ) ",
                    r"   > z <  ",
                    r"  /|   |\ ",
                    r"    zZz   ",
                ],
                &[
                    r"   /\_/\  ",
                    r"  ( -.- ) ",
                    r"   > z <  ",
                    r"  /|   |\ ",
                    r"   zZzZ   ",
                ],
            ],
        }
    }

    fn ticks_per_frame(&self) -> u32 {
        match self {
            PetState::Idle => 8,
            PetState::Typing => 4,
            PetState::TypingFast => 2,
            PetState::Thinking => 5,
            PetState::Happy => 3,
            PetState::Error => 6,
            PetState::Sleeping => 10,
        }
    }

    fn label(&self) -> &str {
        match self {
            PetState::Idle => "Idle",
            PetState::Typing => "Watching...",
            PetState::TypingFast => "Excited!!",
            PetState::Thinking => "Thinking...",
            PetState::Happy => "Happy!",
            PetState::Error => "Oh no...",
            PetState::Sleeping => "zzZ...",
        }
    }

    fn color(&self) -> Color {
        match self {
            PetState::Idle => Color::White,
            PetState::Typing => Color::Cyan,
            PetState::TypingFast => Color::Magenta,
            PetState::Thinking => Color::Yellow,
            PetState::Happy => Color::Green,
            PetState::Error => Color::Red,
            PetState::Sleeping => Color::DarkGray,
        }
    }

    fn current_frame(&self, tick: u32) -> ArtFrame {
        let frames = self.frames();
        let idx = (tick / self.ticks_per_frame()) as usize % frames.len();
        frames[idx]
    }
}

// ── Built-in Header Widgets ─────────────────────────────────

pub struct StatsWidget;

impl HeaderWidget for StatsWidget {
    fn id(&self) -> &str {
        "stats"
    }
    fn preferred_width(&self) -> Option<u16> {
        None
    }

    fn render(&self, f: &mut Frame, area: Rect, ctx: &WidgetContext) {
        let stats = ctx.stats;
        let status_line = if ctx.processing {
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    " PROCESSING ",
                    Style::default().bg(Color::Yellow).fg(Color::Black),
                ),
            ])
        } else {
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    " READY ",
                    Style::default().bg(Color::Green).fg(Color::Black),
                ),
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
                Span::styled(
                    format_token_count(stats.total_input_tokens),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled("  Out: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format_token_count(stats.total_output_tokens),
                    Style::default().fg(Color::Magenta),
                ),
            ]),
            Line::from(vec![
                Span::styled("  Requests: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{}", stats.request_count),
                    Style::default().fg(Color::White),
                ),
                Span::styled("  Day: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{}", usage_days),
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "  Shortcuts",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::UNDERLINED),
            )),
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

pub struct PetWidget;

impl HeaderWidget for PetWidget {
    fn id(&self) -> &str {
        "pet"
    }
    fn preferred_width(&self) -> Option<u16> {
        Some(20)
    }

    fn render(&self, f: &mut Frame, area: Rect, ctx: &WidgetContext) {
        let state = &ctx.pet_state;
        let art_color = state.color();
        let frame = state.current_frame(ctx.anim_tick);

        let mut lines: Vec<Line> = Vec::new();
        for art_line in frame {
            lines.push(Line::from(Span::styled(
                *art_line,
                Style::default().fg(art_color),
            )));
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

// ── Per-session tab state ───────────────────────────────────

struct SessionTab {
    id: String,
    name: String,
    messages: Vec<String>,
    scroll_offset: usize,
    follow_tail: bool,
    processing: bool,
    pet_state: PetState,
    streaming_message_idx: Option<usize>,
    tool_progress_idx: Option<usize>,
    cached_stats: SessionStats,
    agent: Option<Agent>,
    event_rx: Option<tokio::sync::mpsc::UnboundedReceiver<AgentEvent>>,
    agent_handle: Option<tokio::task::JoinHandle<Result<Agent>>>,
    input: String,
    cursor_position: usize,
    pending_messages: VecDeque<String>,
}

impl SessionTab {
    fn new(id: String, name: String, agent: Agent) -> Self {
        let stats = agent.stats.clone();
        Self {
            id,
            name,
            messages: vec!["Welcome to miniclaw! Type your message or /help for commands.".into()],
            scroll_offset: 0,
            follow_tail: true,
            processing: false,
            pet_state: PetState::Idle,
            streaming_message_idx: None,
            tool_progress_idx: None,
            cached_stats: stats,
            agent: Some(agent),
            event_rx: None,
            agent_handle: None,
            input: String::new(),
            cursor_position: 0,
            pending_messages: VecDeque::new(),
        }
    }

    fn byte_index(&self) -> usize {
        self.input
            .char_indices()
            .nth(self.cursor_position)
            .map_or(self.input.len(), |(i, _)| i)
    }

    fn char_count(&self) -> usize {
        self.input.chars().count()
    }

    fn send_next_pending(&mut self) {
        if let Some(msg) = self.pending_messages.pop_front() {
            self.messages.push(format!("You: {}", msg));
            self.processing = true;
            self.pet_state = PetState::Thinking;
            self.follow_tail = true;

            if let Some(mut moved_agent) = self.agent.take() {
                let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                self.event_rx = Some(rx);
                self.agent_handle = Some(tokio::spawn(async move {
                    let result = moved_agent.process_message(&msg, Some(tx)).await;
                    result.map(|_| moved_agent)
                }));
            }
            self.auto_save();
        }
    }

    fn to_session_data(&self) -> SessionData {
        let agent_messages = self
            .agent
            .as_ref()
            .map(|a| a.history().to_vec())
            .unwrap_or_default();
        SessionData {
            id: self.id.clone(),
            name: self.name.clone(),
            created_at: session::now_timestamp(),
            agent_messages,
            ui_messages: self.messages.clone(),
            stats: SessionStatsData::from(&self.cached_stats),
        }
    }

    fn auto_save(&self) {
        let data = self.to_session_data();
        let _ = session::save_session(&data);
    }

    fn handle_agent_event(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::StreamDelta(delta) => {
                if let Some(idx) = self.streaming_message_idx {
                    self.messages[idx].push_str(&delta);
                } else {
                    self.messages.push(format!("Assistant: {}", delta));
                    self.streaming_message_idx = Some(self.messages.len() - 1);
                }
                if self.follow_tail {
                    self.scroll_offset = usize::MAX;
                }
            }
            AgentEvent::LlmText(text) => {
                self.messages.push(format!(
                    "  \u{1f4ad} {}",
                    text.lines()
                        .next()
                        .unwrap_or("")
                        .chars()
                        .take(80)
                        .collect::<String>()
                ));
            }
            AgentEvent::ToolStart { name, arguments } => {
                self.streaming_message_idx = None;
                let text = tool_display_text(&name, &arguments, true);
                self.messages.push(text);
                self.tool_progress_idx = Some(self.messages.len() - 1);
            }
            AgentEvent::ToolEnd {
                name,
                arguments,
                success,
            } => {
                let text = if success {
                    tool_display_text(&name, &arguments, false)
                } else {
                    tool_display_text_error(&name, &arguments)
                };
                if let Some(idx) = self.tool_progress_idx.take() {
                    self.messages[idx] = text;
                } else {
                    self.messages.push(text);
                }
            }
            AgentEvent::Done(response) => {
                self.tool_progress_idx = None;
                if self.streaming_message_idx.is_some() {
                    self.streaming_message_idx = None;
                } else if !response.is_empty() {
                    self.messages.push(format!("Assistant: {}", response));
                }
                self.pet_state = PetState::Happy;
                self.processing = false;
                self.follow_tail = true;
            }
            AgentEvent::Error(e) => {
                self.streaming_message_idx = None;
                self.tool_progress_idx = None;
                self.messages.push(format!("Error: {}", e));
                self.pet_state = PetState::Error;
                self.processing = false;
                self.follow_tail = true;
            }
        }
    }
}

fn tool_display_text(name: &str, arguments: &str, in_progress: bool) -> String {
    let args: serde_json::Value =
        serde_json::from_str(arguments).unwrap_or(serde_json::Value::Null);
    let (action, target) = match name {
        "read_file" => {
            let path = args["path"].as_str().unwrap_or("?");
            if in_progress {
                ("读取文件", path.to_string())
            } else {
                ("已读取", path.to_string())
            }
        }
        "write_file" => {
            let path = args["path"].as_str().unwrap_or("?");
            if in_progress {
                ("写入文件", path.to_string())
            } else {
                ("已写入", path.to_string())
            }
        }
        "list_directory" => {
            let path = args["path"].as_str().unwrap_or(".");
            if in_progress {
                ("浏览目录", path.to_string())
            } else {
                ("已浏览", path.to_string())
            }
        }
        other => {
            if in_progress {
                ("调用", other.to_string())
            } else {
                ("完成", other.to_string())
            }
        }
    };
    if in_progress {
        format!("TOOL_PROGRESS:⚡ {} {} ...", action, target)
    } else {
        format!("TOOL_DONE:✓ {} {}", action, target)
    }
}

fn tool_display_text_error(name: &str, arguments: &str) -> String {
    let args: serde_json::Value =
        serde_json::from_str(arguments).unwrap_or(serde_json::Value::Null);
    let target = match name {
        "read_file" | "write_file" => args["path"].as_str().unwrap_or("?").to_string(),
        "list_directory" => args["path"].as_str().unwrap_or(".").to_string(),
        other => other.to_string(),
    };
    format!("TOOL_ERROR:✗ {} {} 失败", name, target)
}

// ── TUI State ───────────────────────────────────────────────

const HEADER_HEIGHT: u16 = 10;
const TAB_BAR_HEIGHT: u16 = 1;
const TYPING_FAST_THRESHOLD: u32 = 15;
const TYPING_DECAY_PER_TICK: u32 = 1;
const TYPING_BOOST_PER_KEY: u32 = 4;

pub struct RatatuiUi {
    anim_tick: u32,
    idle_ticks: u32,
    typing_intensity: u32,
    header_widgets: Vec<Box<dyn HeaderWidget>>,
    first_use_date: Option<chrono::NaiveDate>,
    autocomplete: SlashAutocomplete,
    tabs: Vec<SessionTab>,
    active_tab: usize,
    config: AppConfig,
    project_root: PathBuf,
    tab_bar_rect: Rect,
    session_rects: Vec<Rect>,
}

impl RatatuiUi {
    pub fn new(config: AppConfig, project_root: PathBuf) -> Self {
        let mut header_widgets: Vec<Box<dyn HeaderWidget>> = Vec::new();
        if config.ui.show_stats {
            header_widgets.push(Box::new(StatsWidget));
        }
        if config.ui.show_pet {
            header_widgets.push(Box::new(PetWidget));
        }

        Self {
            anim_tick: 0,
            idle_ticks: 0,
            typing_intensity: 0,
            header_widgets,
            first_use_date: ensure_first_use_date(),
            autocomplete: SlashAutocomplete::new(),
            tabs: Vec::new(),
            active_tab: 0,
            config,
            project_root,
            tab_bar_rect: Rect::default(),
            session_rects: Vec::new(),
        }
    }

    fn active(&self) -> &SessionTab {
        &self.tabs[self.active_tab]
    }

    fn active_mut(&mut self) -> &mut SessionTab {
        &mut self.tabs[self.active_tab]
    }

    fn create_new_tab(&mut self, name: Option<String>) -> Result<()> {
        let id = session::generate_session_id();
        let tab_name = name.unwrap_or_else(|| format!("Session {}", self.tabs.len() + 1));
        let agent = Agent::create(&self.config, &self.project_root)?;
        self.tabs.push(SessionTab::new(id, tab_name, agent));
        self.active_tab = self.tabs.len() - 1;
        Ok(())
    }

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

    fn handle_key_event(&mut self, key: KeyEvent) {
        let tab = self.active_mut();
        match key.code {
            KeyCode::Char(c) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    match c {
                        'u' => {
                            tab.input.clear();
                            tab.cursor_position = 0;
                        }
                        'k' => {
                            let b = tab.byte_index();
                            tab.input.drain(b..);
                        }
                        'w' => {
                            let end = tab.byte_index();
                            let chars: Vec<char> = tab.input.chars().collect();
                            while tab.cursor_position > 0
                                && chars[tab.cursor_position - 1].is_whitespace()
                            {
                                tab.cursor_position -= 1;
                            }
                            while tab.cursor_position > 0
                                && !chars[tab.cursor_position - 1].is_whitespace()
                            {
                                tab.cursor_position -= 1;
                            }
                            let start = tab.byte_index();
                            tab.input.drain(start..end);
                        }
                        _ => {}
                    }
                } else {
                    let b = tab.byte_index();
                    tab.input.insert(b, c);
                    tab.cursor_position += 1;
                }
            }
            KeyCode::Backspace => {
                if tab.cursor_position > 0 {
                    tab.cursor_position -= 1;
                    let b = tab.byte_index();
                    tab.input.remove(b);
                }
            }
            KeyCode::Delete => {
                if tab.cursor_position < tab.char_count() {
                    let b = tab.byte_index();
                    tab.input.remove(b);
                }
            }
            KeyCode::Left if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if tab.cursor_position > 0 {
                    tab.cursor_position -= 1;
                }
            }
            KeyCode::Right if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if tab.cursor_position < tab.char_count() {
                    tab.cursor_position += 1;
                }
            }
            KeyCode::Home => {
                tab.cursor_position = 0;
            }
            KeyCode::End => {
                tab.cursor_position = tab.char_count();
            }
            _ => {}
        }
        let input_snapshot = self.active().input.clone();
        self.autocomplete.update_filter(&input_snapshot);
    }

    fn apply_autocomplete_selection(&mut self) {
        if let Some(cmd) = self.autocomplete.selected_command() {
            let tab = self.active_mut();
            tab.input = cmd.to_string();
            tab.cursor_position = tab.input.chars().count();
            self.autocomplete.dismiss();
        }
    }

    fn build_conversation_lines(messages: &[String]) -> Vec<Line<'static>> {
        let mut text_lines = Vec::new();
        for msg in messages {
            if let Some(rest) = msg.strip_prefix("You: ") {
                text_lines.push(Line::from(vec![
                    Span::styled("You: ".to_string(), Style::default().fg(Color::Green)),
                    Span::raw(rest.to_string()),
                ]));
                text_lines.push(Line::from(""));
            } else if let Some(rest) = msg.strip_prefix("Assistant: ") {
                text_lines.push(Line::from(Span::styled(
                    "Assistant:".to_string(),
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                )));
                let md_lines = crate::ui::markdown::markdown_to_lines(rest);
                text_lines.extend(md_lines);
            } else if let Some(rest) = msg.strip_prefix("TOOL_PROGRESS:") {
                text_lines.push(Line::from(Span::styled(
                    format!("  {}", rest),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::ITALIC),
                )));
            } else if let Some(rest) = msg.strip_prefix("TOOL_DONE:") {
                text_lines.push(Line::from(Span::styled(
                    format!("  {}", rest),
                    Style::default().fg(Color::Cyan),
                )));
            } else if let Some(rest) = msg.strip_prefix("TOOL_ERROR:") {
                text_lines.push(Line::from(Span::styled(
                    format!("  {}", rest),
                    Style::default().fg(Color::Red),
                )));
            } else {
                text_lines.push(Line::from(msg.clone()));
                text_lines.push(Line::from(""));
            }
        }
        text_lines
    }

    fn estimate_rendered_lines(lines: &[Line], wrap_width: usize) -> usize {
        if wrap_width == 0 {
            return lines.len();
        }
        lines
            .iter()
            .map(|line| {
                let width: usize = line
                    .spans
                    .iter()
                    .map(|s| {
                        s.content
                            .chars()
                            .map(|c| if c.is_ascii() { 1 } else { 2 })
                            .sum::<usize>()
                    })
                    .sum();
                1usize.max(width.div_ceil(wrap_width))
            })
            .sum()
    }

    fn render_tab_bar(&mut self, f: &mut Frame, area: Rect) {
        self.tab_bar_rect = area;
        let mut spans = Vec::new();
        for (i, tab) in self.tabs.iter().enumerate() {
            let label = if tab.processing {
                format!(" {}⏳ ", tab.name)
            } else {
                format!(" {} ", tab.name)
            };
            if i == self.active_tab {
                spans.push(Span::styled(
                    label,
                    Style::default()
                        .bg(Color::Cyan)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                spans.push(Span::styled(label, Style::default().fg(Color::DarkGray)));
            }
            if i + 1 < self.tabs.len() {
                spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
            }
        }
        spans.push(Span::styled("  [+]", Style::default().fg(Color::Green)));
        let line = Line::from(spans);
        let widget = Paragraph::new(vec![line]).style(Style::default().bg(Color::Black));
        f.render_widget(widget, area);
    }

    fn render_conversations(&mut self, f: &mut Frame, area: Rect) {
        let tab_count = self.tabs.len();
        let active = self.active_tab;
        if tab_count == 1 {
            self.session_rects = vec![area];
            Self::render_single_conversation(&mut self.tabs[0], true, f, area);
            return;
        }

        let constraints: Vec<Constraint> = self
            .tabs
            .iter()
            .map(|_| Constraint::Ratio(1, tab_count as u32))
            .collect();
        let cols = Layout::horizontal(constraints).split(area);
        self.session_rects = cols.to_vec();

        for (i, tab) in self.tabs.iter_mut().enumerate() {
            let is_active = i == active;
            Self::render_single_conversation(tab, is_active, f, cols[i]);
        }
    }

    fn render_single_conversation(
        tab: &mut SessionTab,
        is_active: bool,
        f: &mut Frame,
        area: Rect,
    ) {
        let text_lines = Self::build_conversation_lines(&tab.messages);
        let visible_height = area.height.saturating_sub(2) as usize;
        let wrap_width = area.width.saturating_sub(2) as usize;
        let total_rendered = Self::estimate_rendered_lines(&text_lines, wrap_width);
        let max_scroll = total_rendered.saturating_sub(visible_height);

        if tab.follow_tail {
            tab.scroll_offset = max_scroll;
        } else {
            tab.scroll_offset = tab.scroll_offset.min(max_scroll);
            if tab.scroll_offset >= max_scroll {
                tab.follow_tail = true;
            }
        }
        let scroll = tab.scroll_offset;

        let border_color = if is_active {
            Color::Cyan
        } else {
            Color::DarkGray
        };
        let title_style = if is_active {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let title = if tab.processing {
            format!(" {} ⏳ ", tab.name)
        } else {
            format!(" {} ", tab.name)
        };

        let p = Paragraph::new(text_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .title_style(title_style)
                    .border_style(Style::default().fg(border_color)),
            )
            .wrap(Wrap { trim: true })
            .scroll((scroll as u16, 0));
        f.render_widget(p, area);
    }

    fn render_input(&self, f: &mut Frame, area: Rect) {
        let tab = self.active();
        let pending_hint = if !tab.pending_messages.is_empty() {
            format!(" [{} pending]", tab.pending_messages.len())
        } else {
            String::new()
        };
        let title = format!("Input (Shift+Enter newline){}", pending_hint);

        let p = Paragraph::new(tab.input.as_str())
            .block(Block::default().borders(Borders::ALL).title(title))
            .wrap(Wrap { trim: false });
        f.render_widget(p, area);

        let (cursor_row, cursor_col) = Self::cursor_row_col(&tab.input, tab.cursor_position);
        f.set_cursor_position((
            area.x + cursor_col as u16 + 1,
            area.y + cursor_row as u16 + 1,
        ));
    }

    fn cursor_row_col(input: &str, cursor_pos: usize) -> (usize, usize) {
        let before_cursor: String = input.chars().take(cursor_pos).collect();
        let row = before_cursor.matches('\n').count();
        let last_newline = before_cursor.rfind('\n');
        let col_str = match last_newline {
            Some(pos) => &before_cursor[pos + 1..],
            None => &before_cursor,
        };
        let col: usize = col_str
            .chars()
            .map(|c| if c.is_ascii() { 1 } else { 2 })
            .sum();
        (row, col)
    }

    fn input_area_height(&self) -> u16 {
        let tab = self.active();
        let line_count = tab.input.matches('\n').count() + 1;
        (line_count as u16 + 2).max(3).min(8)
    }

    fn render_autocomplete(&self, f: &mut Frame, input_area: Rect) {
        if !self.autocomplete.visible || self.active().processing {
            return;
        }

        let item_count = self.autocomplete.filtered.len() as u16;
        let popup_height = item_count + 2;
        let popup_width = 50u16.min(input_area.width);

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
                        format!(" {:<12}", cmd.name),
                        Style::default()
                            .fg(fg_name)
                            .bg(bg)
                            .add_modifier(if is_selected {
                                Modifier::BOLD
                            } else {
                                Modifier::empty()
                            }),
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

    fn render_header(&self, f: &mut Frame, area: Rect) {
        if self.header_widgets.is_empty() {
            return;
        }
        let tab = self.active();
        let ctx = WidgetContext {
            stats: &tab.cached_stats,
            messages: &tab.messages,
            processing: tab.processing,
            anim_tick: self.anim_tick,
            pet_state: tab.pet_state,
            idle_ticks: self.idle_ticks,
            typing_intensity: self.typing_intensity,
            first_use_date: self.first_use_date,
        };

        let constraints: Vec<Constraint> = self
            .header_widgets
            .iter()
            .map(|w| match w.preferred_width() {
                Some(width) => Constraint::Length(width),
                None => Constraint::Min(20),
            })
            .collect();

        let cols = Layout::horizontal(constraints).split(area);
        for (i, widget) in self.header_widgets.iter().enumerate() {
            if i < cols.len() {
                widget.render(f, cols[i], &ctx);
            }
        }
    }

    fn draw_ui(&mut self, f: &mut Frame) {
        let area = f.area();
        let header_h = if self.header_widgets.is_empty() {
            0
        } else {
            HEADER_HEIGHT
        };
        let show_tabs = self.tabs.len() > 1;
        let tab_h = if show_tabs { TAB_BAR_HEIGHT } else { 0 };
        let input_h = self.input_area_height();

        let rows = Layout::vertical([
            Constraint::Length(header_h),
            Constraint::Length(tab_h),
            Constraint::Min(4),
            Constraint::Length(input_h),
        ])
        .split(area);

        if header_h > 0 {
            self.render_header(f, rows[0]);
        }
        if show_tabs {
            self.render_tab_bar(f, rows[1]);
        }
        self.render_conversations(f, rows[2]);
        self.render_input(f, rows[3]);
        self.render_autocomplete(f, rows[3]);
    }

    fn handle_command(&mut self, cmd: &str) -> Option<UiExitAction> {
        let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
        let command = parts[0];
        let arg = parts.get(1).map(|s| s.trim()).unwrap_or("");

        match command {
            "/quit" | "/exit" => {
                for tab in &self.tabs {
                    tab.auto_save();
                }
                return Some(UiExitAction::Quit);
            }
            "/clear" => {
                if let Some(agent) = self.active_mut().agent.as_mut() {
                    agent.clear_history();
                }
                self.active_mut().messages.clear();
                self.active_mut()
                    .messages
                    .push("Conversation cleared.".into());
                self.active_mut().scroll_offset = 0;
                self.active_mut().follow_tail = true;
            }
            "/new" => {
                let name = if arg.is_empty() {
                    None
                } else {
                    Some(arg.to_string())
                };
                match self.create_new_tab(name) {
                    Ok(()) => {
                        let n = self.active().name.clone();
                        self.active_mut()
                            .messages
                            .push(format!("[Created new session: {}]", n));
                    }
                    Err(e) => {
                        self.active_mut()
                            .messages
                            .push(format!("Error creating session: {}", e));
                    }
                }
            }
            "/close" => {
                if self.tabs.len() <= 1 {
                    self.active_mut()
                        .messages
                        .push("[Cannot close the last session]".into());
                } else {
                    self.tabs.remove(self.active_tab);
                    if self.active_tab >= self.tabs.len() {
                        self.active_tab = self.tabs.len() - 1;
                    }
                }
            }
            "/rename" => {
                if arg.is_empty() {
                    self.active_mut()
                        .messages
                        .push("Usage: /rename <name>".into());
                } else {
                    self.active_mut().name = arg.to_string();
                    self.active_mut()
                        .messages
                        .push(format!("[Session renamed to: {}]", arg));
                }
            }
            "/sessions" => match session::list_sessions() {
                Ok(sessions) if sessions.is_empty() => {
                    self.active_mut()
                        .messages
                        .push("[No saved sessions]".into());
                }
                Ok(sessions) => {
                    self.active_mut()
                        .messages
                        .push("--- Saved Sessions ---".into());
                    for s in &sessions {
                        self.active_mut().messages.push(format!(
                            "  {} | {} | {} | msgs: {}",
                            s.id,
                            s.name,
                            s.created_at,
                            s.ui_messages.len()
                        ));
                    }
                }
                Err(e) => {
                    self.active_mut()
                        .messages
                        .push(format!("Error listing sessions: {}", e));
                }
            },
            "/save" => {
                let name = if arg.is_empty() {
                    None
                } else {
                    Some(arg.to_string())
                };
                if let Some(n) = name {
                    self.active_mut().name = n;
                }
                let data = self.active().to_session_data();
                match session::save_session(&data) {
                    Ok(path) => {
                        self.active_mut().messages.push(format!(
                            "[Session saved: {} → {}]",
                            data.name,
                            path.display()
                        ));
                    }
                    Err(e) => {
                        self.active_mut()
                            .messages
                            .push(format!("Error saving session: {}", e));
                    }
                }
            }
            "/load" => {
                if arg.is_empty() {
                    self.active_mut()
                        .messages
                        .push("Usage: /load <session_id>".into());
                } else {
                    match self.load_session_as_tab(arg) {
                        Ok(()) => {}
                        Err(e) => {
                            self.active_mut()
                                .messages
                                .push(format!("Error loading session: {}", e));
                        }
                    }
                }
            }
            "/export" => {
                if arg.is_empty() {
                    self.active_mut()
                        .messages
                        .push("Usage: /export <path>".into());
                } else {
                    let data = self.active().to_session_data();
                    match session::export_session(&data, std::path::Path::new(arg)) {
                        Ok(()) => {
                            self.active_mut()
                                .messages
                                .push(format!("[Session exported to {}]", arg));
                        }
                        Err(e) => {
                            self.active_mut()
                                .messages
                                .push(format!("Error exporting: {}", e));
                        }
                    }
                }
            }
            "/import" => {
                if arg.is_empty() {
                    self.active_mut()
                        .messages
                        .push("Usage: /import <path>".into());
                } else {
                    match self.import_session_as_tab(arg) {
                        Ok(()) => {}
                        Err(e) => {
                            self.active_mut()
                                .messages
                                .push(format!("Error importing: {}", e));
                        }
                    }
                }
            }
            "/stats" => {
                let visible = self.toggle_widget("stats");
                self.active_mut().messages.push(format!(
                    "[Stats panel {}]",
                    if visible { "enabled" } else { "disabled" }
                ));
            }
            "/pet" => {
                let visible = self.toggle_widget("pet");
                self.active_mut().messages.push(format!(
                    "[Pet panel {}]",
                    if visible { "enabled" } else { "disabled" }
                ));
            }
            "/help" => {
                let help = [
                    "--- Commands ---",
                    "  /help              Show available commands",
                    "  /clear             Clear conversation history",
                    "  /new [name]        Create new session tab",
                    "  /close             Close current session tab",
                    "  /rename <name>     Rename current session",
                    "  /save [name]       Save current session",
                    "  /load <id>         Load saved session",
                    "  /sessions          List saved sessions",
                    "  /export <path>     Export session to file",
                    "  /import <path>     Import session from file",
                    "  /stats             Toggle stats panel",
                    "  /pet               Toggle pet panel",
                    "  /quit              Exit the program",
                    "",
                    "  Ctrl+Left/Right    Switch session tabs",
                    "  Ctrl+C             Exit the program",
                ];
                for line in help {
                    self.active_mut().messages.push(line.to_string());
                }
            }
            other => {
                self.active_mut().messages.push(format!(
                    "Unknown command: {}. Type /help for commands.",
                    other
                ));
            }
        }
        None
    }

    fn load_session_as_tab(&mut self, id: &str) -> Result<()> {
        let data = session::load_session(id)?;
        let mut agent = Agent::create(&self.config, &self.project_root)?;
        agent.set_messages(data.agent_messages);
        agent.stats = data.stats.to_session_stats();
        let mut tab = SessionTab::new(data.id, data.name.clone(), agent);
        tab.messages = data.ui_messages;
        tab.cached_stats = data.stats.to_session_stats();
        self.tabs.push(tab);
        self.active_tab = self.tabs.len() - 1;
        self.active_mut()
            .messages
            .push(format!("[Loaded session: {}]", data.name));
        Ok(())
    }

    fn import_session_as_tab(&mut self, path: &str) -> Result<()> {
        let data = session::import_session(std::path::Path::new(path))?;
        let mut agent = Agent::create(&self.config, &self.project_root)?;
        agent.set_messages(data.agent_messages);
        agent.stats = data.stats.to_session_stats();
        let mut tab = SessionTab::new(data.id, data.name.clone(), agent);
        tab.messages = data.ui_messages;
        tab.cached_stats = data.stats.to_session_stats();
        self.tabs.push(tab);
        self.active_tab = self.tabs.len() - 1;
        self.active_mut()
            .messages
            .push(format!("[Imported session: {}]", data.name));
        Ok(())
    }

    fn handle_mouse_tab_click(&mut self, x: u16) {
        let mut current_x = 0u16;
        for (i, tab) in self.tabs.iter().enumerate() {
            let label_width = if tab.processing {
                tab.name.chars().count() + 4
            } else {
                tab.name.chars().count() + 2
            } as u16;
            if x >= current_x && x < current_x + label_width {
                self.active_tab = i;
                return;
            }
            current_x += label_width;
            current_x += 3; // separator " │ "
        }
    }

    pub async fn run(mut self, agent: Agent) -> Result<UiExitAction> {
        let _ = terminal::disable_raw_mode();
        while event::poll(std::time::Duration::from_millis(5))? {
            let _ = event::read()?;
        }

        crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture)?;

        let mut terminal = ratatui::init();
        let _guard = TerminalGuard;
        let exit_action;

        let id = session::generate_session_id();
        self.tabs
            .push(SessionTab::new(id, "Session 1".into(), agent));

        loop {
            self.anim_tick = self.anim_tick.wrapping_add(1);
            terminal.draw(|f| self.draw_ui(f))?;

            // Process events for ALL tabs
            for tab in &mut self.tabs {
                let mut rx_taken = tab.event_rx.take();
                if let Some(rx) = &mut rx_taken {
                    let mut terminal_reached = false;
                    while let Ok(evt) = rx.try_recv() {
                        let is_terminal = matches!(evt, AgentEvent::Done(_) | AgentEvent::Error(_));
                        tab.handle_agent_event(evt);
                        if is_terminal {
                            terminal_reached = true;
                            break;
                        }
                    }
                    if terminal_reached {
                        if let Some(handle) = tab.agent_handle.take() {
                            match handle.await {
                                Ok(Ok(returned_agent)) => {
                                    tab.cached_stats = returned_agent.stats.clone();
                                    tab.agent = Some(returned_agent);
                                }
                                Ok(Err(e)) => {
                                    tab.messages.push(format!("Error: {}", e));
                                    tab.pet_state = PetState::Error;
                                    tab.processing = false;
                                }
                                Err(e) => {
                                    tab.messages.push(format!("Error: task panicked: {}", e));
                                    tab.pet_state = PetState::Error;
                                    tab.processing = false;
                                }
                            }
                        }
                        tab.auto_save();
                        if !tab.pending_messages.is_empty() {
                            tab.send_next_pending();
                        }
                        // rx dropped (not put back)
                    } else {
                        tab.event_rx = rx_taken;
                    }
                }
            }

            if event::poll(std::time::Duration::from_millis(100))? {
                match event::read()? {
                    Event::Key(key) => {
                        self.idle_ticks = 0;
                        self.typing_intensity = self
                            .typing_intensity
                            .saturating_add(TYPING_BOOST_PER_KEY)
                            .min(40);

                        match key.code {
                            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                exit_action = UiExitAction::Quit;
                                break;
                            }
                            KeyCode::Left if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                if self.active_tab > 0 {
                                    self.active_tab -= 1;
                                }
                            }
                            KeyCode::Right if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                if self.active_tab + 1 < self.tabs.len() {
                                    self.active_tab += 1;
                                }
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
                            // Shift+Enter inserts newline
                            KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                                let tab = self.active_mut();
                                let b = tab.byte_index();
                                tab.input.insert(b, '\n');
                                tab.cursor_position += 1;
                                self.autocomplete.dismiss();
                            }
                            KeyCode::Enter => {
                                if self.autocomplete.visible {
                                    self.apply_autocomplete_selection();
                                    let user_input = self.active().input.clone();
                                    let tab = self.active_mut();
                                    tab.input.clear();
                                    tab.cursor_position = 0;
                                    self.autocomplete.dismiss();
                                    if is_slash_command(&user_input) {
                                        if let Some(action) = self.handle_command(&user_input) {
                                            exit_action = action;
                                            break;
                                        }
                                    }
                                    continue;
                                }

                                let input_text = self.active().input.trim().to_string();
                                if !input_text.is_empty() {
                                    let tab = self.active_mut();
                                    tab.input.clear();
                                    tab.cursor_position = 0;
                                    self.autocomplete.dismiss();

                                    if is_slash_command(
                                        input_text.split_whitespace().next().unwrap_or(""),
                                    ) {
                                        if let Some(action) = self.handle_command(&input_text) {
                                            exit_action = action;
                                            break;
                                        }
                                        continue;
                                    }

                                    let tab = self.active_mut();
                                    if tab.processing {
                                        tab.pending_messages.push_back(input_text);
                                    } else {
                                        tab.messages.push(format!("You: {}", input_text));
                                        tab.processing = true;
                                        tab.pet_state = PetState::Thinking;
                                        tab.follow_tail = true;
                                        tab.auto_save();

                                        if let Some(mut moved_agent) = tab.agent.take() {
                                            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                                            tab.event_rx = Some(rx);
                                            let input_clone = input_text.clone();
                                            tab.agent_handle = Some(tokio::spawn(async move {
                                                let result = moved_agent
                                                    .process_message(&input_clone, Some(tx))
                                                    .await;
                                                result.map(|_| moved_agent)
                                            }));
                                        }
                                    }
                                }
                            }
                            // PageUp/PageDown for fast scroll
                            KeyCode::PageUp => {
                                self.active_mut().follow_tail = false;
                                let off = self.active().scroll_offset;
                                self.active_mut().scroll_offset = off.saturating_sub(10);
                            }
                            KeyCode::PageDown => {
                                let tab = self.active_mut();
                                tab.scroll_offset += 10;
                            }
                            _ => {
                                self.handle_key_event(key);
                            }
                        }
                    }
                    Event::Mouse(mouse) => match mouse.kind {
                        MouseEventKind::Down(MouseButton::Left) => {
                            let tab_bar = self.tab_bar_rect;
                            if self.tabs.len() > 1
                                && mouse.row == tab_bar.y
                                && mouse.column >= tab_bar.x
                                && mouse.column < tab_bar.x + tab_bar.width
                            {
                                self.handle_mouse_tab_click(mouse.column - tab_bar.x);
                            }
                            for (i, rect) in self.session_rects.iter().enumerate() {
                                if mouse.row >= rect.y
                                    && mouse.row < rect.y + rect.height
                                    && mouse.column >= rect.x
                                    && mouse.column < rect.x + rect.width
                                {
                                    self.active_tab = i;
                                    break;
                                }
                            }
                        }
                        MouseEventKind::ScrollUp => {
                            self.active_mut().follow_tail = false;
                            let off = self.active().scroll_offset;
                            self.active_mut().scroll_offset = off.saturating_sub(3);
                        }
                        MouseEventKind::ScrollDown => {
                            self.active_mut().scroll_offset += 3;
                        }
                        _ => {}
                    },
                    _ => {}
                }
            } else {
                self.idle_ticks += 1;
                self.typing_intensity = self.typing_intensity.saturating_sub(TYPING_DECAY_PER_TICK);
            }

            // Pet state machine for active tab
            {
                let ti = self.typing_intensity;
                let idle = self.idle_ticks;
                let input_empty = self.tabs[self.active_tab].input.is_empty();
                let tab = &mut self.tabs[self.active_tab];
                if !tab.processing {
                    if ti > TYPING_FAST_THRESHOLD {
                        tab.pet_state = PetState::TypingFast;
                    } else if ti > 0 && !input_empty {
                        tab.pet_state = PetState::Typing;
                    } else if idle > 300 {
                        tab.pet_state = PetState::Sleeping;
                    } else if ((tab.pet_state == PetState::Happy
                        || tab.pet_state == PetState::Error)
                        && idle > 50)
                        || ((tab.pet_state == PetState::Typing
                            || tab.pet_state == PetState::TypingFast)
                            && ti == 0)
                    {
                        tab.pet_state = PetState::Idle;
                    }
                }
            }
        }

        drop(_guard);
        Ok(exit_action)
    }
}
