use crate::themes::Theme;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

/// Truncate to a display width, appending an ellipsis when needed.
pub fn truncate(text: &str, max_width: usize) -> String {
    if max_width == 0 { return String::new(); }
    if UnicodeWidthStr::width(text) <= max_width { return text.to_string(); }
    if max_width == 1 { return "…".into(); }
    let mut out = String::new();
    let mut width = 0;
    for c in text.chars() {
        let w = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        if width + w > max_width - 1 { break; }
        out.push(c);
        width += w;
    }
    out.push('…');
    out
}

pub fn pad(text: &str, width: usize) -> String {
    let w = UnicodeWidthStr::width(text);
    if w >= width { return text.to_string(); }
    format!("{}{}", text, " ".repeat(width - w))
}

pub fn section_line(title: &str, theme: &Theme, width: usize) -> Line<'static> {
    let label = format!(" {}", title.to_uppercase());
    let mut spans = vec![
        Span::styled(label, Style::default().fg(theme.side_bar_title).add_modifier(Modifier::BOLD)),
    ];
    let used = UnicodeWidthStr::width(spans[0].content.as_ref()) + 1;
    if width > used {
        spans.push(Span::styled(" ".repeat(width - used), Style::default().bg(theme.side_bar_section_header_bg)));
    }
    Line::from(spans)
}

pub fn file_icon_color(theme: &Theme, is_dir: bool) -> Style {
    if is_dir {
        Style::default().fg(theme.syntax_variable)
    } else {
        Style::default().fg(theme.syntax_type)
    }
}

pub fn key_hint(text: &str, theme: &Theme) -> Span<'static> {
    Span::styled(text.to_string(), Style::default().fg(theme.panel_inactive))
}
