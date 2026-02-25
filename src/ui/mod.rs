//! UI Module - TUI-only user interface for miniclaw.

use ratatui::Frame;

use crate::agent::Agent;

/// What should happen when the UI exits its run loop.
#[derive(Debug, Clone)]
pub enum UiExitAction {
    Quit,
}

/// Context passed to header widgets each render frame.
pub struct WidgetContext<'a> {
    pub agent: &'a Agent,
    pub messages: &'a [String],
    pub processing: bool,
    pub anim_tick: u32,
    pub pet_state: PetState,
    pub idle_ticks: u32,
    pub typing_intensity: u32,
    pub first_use_date: Option<chrono::NaiveDate>,
}

/// Pluggable header widget trait.
pub trait HeaderWidget {
    fn id(&self) -> &str;
    fn preferred_width(&self) -> Option<u16>;
    fn render(&self, f: &mut Frame, area: ratatui::layout::Rect, ctx: &WidgetContext);
}

pub use ratatui_ui::PetState;

pub mod ratatui_ui;
