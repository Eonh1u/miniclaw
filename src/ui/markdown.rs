//! Markdown to ratatui styled text converter.
//!
//! Uses pulldown-cmark to parse Markdown and produces `Vec<Line>` with
//! appropriate colors and modifiers for terminal rendering.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub fn markdown_to_lines(md: &str) -> Vec<Line<'static>> {
    let opts = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES;
    let parser = Parser::new_ext(md, opts);

    let mut renderer = MdRenderer::new();
    for event in parser {
        renderer.process(event);
    }
    renderer.flush_line();
    renderer.lines
}

struct MdRenderer {
    lines: Vec<Line<'static>>,
    current_spans: Vec<Span<'static>>,

    bold: bool,
    italic: bool,
    in_code_span: bool,
    in_code_block: bool,
    in_heading: u8,

    list_stack: Vec<ListKind>,
}

#[derive(Clone)]
enum ListKind {
    Unordered,
    Ordered(u64),
}

impl MdRenderer {
    fn new() -> Self {
        Self {
            lines: Vec::new(),
            current_spans: Vec::new(),
            bold: false,
            italic: false,
            in_code_span: false,
            in_code_block: false,
            in_heading: 0,
            list_stack: Vec::new(),
        }
    }

    fn current_style(&self) -> Style {
        if self.in_code_block {
            return Style::default().fg(Color::Green);
        }
        if self.in_code_span {
            return Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD);
        }
        if self.in_heading > 0 {
            let color = match self.in_heading {
                1 => Color::Yellow,
                2 => Color::Cyan,
                _ => Color::Blue,
            };
            return Style::default().fg(color).add_modifier(Modifier::BOLD);
        }

        let mut style = Style::default();
        if self.bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        if self.italic {
            style = style.add_modifier(Modifier::ITALIC);
        }
        style
    }

    fn push_text(&mut self, text: &str) {
        if self.in_code_block {
            for (i, code_line) in text.split('\n').enumerate() {
                if i > 0 {
                    self.flush_line();
                    self.current_spans
                        .push(Span::styled("  ".to_string(), Style::default()));
                }
                if !code_line.is_empty() {
                    self.current_spans.push(Span::styled(
                        format!("  {}", code_line),
                        self.current_style(),
                    ));
                }
            }
            return;
        }

        let style = self.current_style();
        for (i, segment) in text.split('\n').enumerate() {
            if i > 0 {
                self.flush_line();
            }
            if !segment.is_empty() {
                self.current_spans
                    .push(Span::styled(segment.to_string(), style));
            }
        }
    }

    fn flush_line(&mut self) {
        let spans = std::mem::take(&mut self.current_spans);
        self.lines.push(Line::from(spans));
    }

    fn list_indent(&self) -> String {
        "  ".repeat(self.list_stack.len().saturating_sub(1))
    }

    fn process(&mut self, event: Event) {
        match event {
            Event::Start(tag) => self.start_tag(tag),
            Event::End(tag) => self.end_tag(tag),
            Event::Text(text) => self.push_text(&text),
            Event::Code(code) => {
                let style = Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD);
                self.current_spans
                    .push(Span::styled(format!("`{}`", code), style));
            }
            Event::SoftBreak => {
                self.current_spans.push(Span::raw(" ".to_string()));
            }
            Event::HardBreak => {
                self.flush_line();
            }
            Event::Rule => {
                self.flush_line();
                self.lines.push(Line::from(Span::styled(
                    "─".repeat(40),
                    Style::default().fg(Color::DarkGray),
                )));
                self.lines.push(Line::from(""));
            }
            _ => {}
        }
    }

    fn start_tag(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => {}
            Tag::Heading { level, .. } => {
                self.in_heading = level as u8;
            }
            Tag::Strong => {
                self.bold = true;
            }
            Tag::Emphasis => {
                self.italic = true;
            }
            Tag::CodeBlock(_) => {
                self.flush_line();
                self.in_code_block = true;
            }
            Tag::List(start) => {
                if self.list_stack.is_empty() && !self.current_spans.is_empty() {
                    self.flush_line();
                }
                let kind = match start {
                    Some(n) => ListKind::Ordered(n),
                    None => ListKind::Unordered,
                };
                self.list_stack.push(kind);
            }
            Tag::Item => {
                let indent = self.list_indent();
                let bullet = match self.list_stack.last() {
                    Some(ListKind::Unordered) => format!("{}  • ", indent),
                    Some(ListKind::Ordered(n)) => {
                        let s = format!("{}  {}. ", indent, n);
                        if let Some(ListKind::Ordered(ref mut n)) = self.list_stack.last_mut() {
                            *n += 1;
                        }
                        s
                    }
                    None => "  ".to_string(),
                };
                self.current_spans
                    .push(Span::styled(bullet, Style::default().fg(Color::DarkGray)));
            }
            Tag::BlockQuote(_) => {
                self.current_spans.push(Span::styled(
                    "│ ".to_string(),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            _ => {}
        }
    }

    fn end_tag(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                self.flush_line();
                self.lines.push(Line::from(""));
            }
            TagEnd::Heading(_) => {
                self.in_heading = 0;
                self.flush_line();
                self.lines.push(Line::from(""));
            }
            TagEnd::Strong => {
                self.bold = false;
            }
            TagEnd::Emphasis => {
                self.italic = false;
            }
            TagEnd::CodeBlock => {
                self.in_code_block = false;
                self.flush_line();
                self.lines.push(Line::from(""));
            }
            TagEnd::List(_) => {
                self.list_stack.pop();
                if self.list_stack.is_empty() {
                    self.lines.push(Line::from(""));
                }
            }
            TagEnd::Item => {
                self.flush_line();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines_to_plain(lines: &[Line]) -> String {
        lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn test_plain_paragraph() {
        let lines = markdown_to_lines("Hello world");
        let text = lines_to_plain(&lines);
        assert!(text.contains("Hello world"));
    }

    #[test]
    fn test_bold_has_modifier() {
        let lines = markdown_to_lines("This is **bold** text");
        let bold_span = lines
            .iter()
            .flat_map(|l| &l.spans)
            .find(|s| s.content.contains("bold"))
            .expect("should have bold span");
        assert!(bold_span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_italic_has_modifier() {
        let lines = markdown_to_lines("This is *italic* text");
        let italic_span = lines
            .iter()
            .flat_map(|l| &l.spans)
            .find(|s| s.content.contains("italic"))
            .expect("should have italic span");
        assert!(italic_span.style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn test_inline_code_styled() {
        let lines = markdown_to_lines("Run `cargo build` now");
        let code_span = lines
            .iter()
            .flat_map(|l| &l.spans)
            .find(|s| s.content.contains("cargo build"))
            .expect("should have code span");
        assert_eq!(code_span.style.fg, Some(Color::Yellow));
        assert!(code_span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_heading_styled() {
        let lines = markdown_to_lines("# Title");
        let title_span = lines
            .iter()
            .flat_map(|l| &l.spans)
            .find(|s| s.content.contains("Title"))
            .expect("should have title span");
        assert_eq!(title_span.style.fg, Some(Color::Yellow));
        assert!(title_span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_unordered_list() {
        let lines = markdown_to_lines("- first\n- second\n- third");
        let text = lines_to_plain(&lines);
        assert!(text.contains("•"));
        assert!(text.contains("first"));
        assert!(text.contains("second"));
        assert!(text.contains("third"));
    }

    #[test]
    fn test_ordered_list() {
        let lines = markdown_to_lines("1. one\n2. two\n3. three");
        let text = lines_to_plain(&lines);
        assert!(text.contains("1."));
        assert!(text.contains("2."));
        assert!(text.contains("one"));
    }

    #[test]
    fn test_code_block() {
        let md = "```rust\nfn main() {}\n```";
        let lines = markdown_to_lines(md);
        let text = lines_to_plain(&lines);
        assert!(text.contains("fn main()"));
        let code_span = lines
            .iter()
            .flat_map(|l| &l.spans)
            .find(|s| s.content.contains("fn main()"))
            .expect("code block span");
        assert_eq!(code_span.style.fg, Some(Color::Green));
    }

    #[test]
    fn test_horizontal_rule() {
        let lines = markdown_to_lines("above\n\n---\n\nbelow");
        let text = lines_to_plain(&lines);
        assert!(text.contains("─"));
    }
}
