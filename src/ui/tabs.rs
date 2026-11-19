use super::icons::icon_span;
use super::style::truncate;
use crate::app::App;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabKind {
    Editor(usize),
    LeftMore,
    RightMore,
}

#[derive(Debug, Clone, Copy)]
pub struct TabLayout {
    pub kind: TabKind,
    pub x: u16,
    pub width: u16,
}

/// Shared between the tab renderer and mouse hit-testing so clicks always
/// select the tab the user sees. Keeps the active tab visible on narrow screens.
pub fn tab_layout(app: &App, area_width: u16) -> Vec<TabLayout> {
    let mut out = Vec::new();
    if app.editors.is_empty() || area_width == 0 { return out; }

    let mut widths: Vec<u16> = app.editors.iter()
        .map(|e| ((e.name().chars().count() + 6).min(24)).max(8) as u16)
        .collect();
    let mut start = 0usize;
    let mut end = app.editors.len();
    let mut left_more = false;
    let mut right_more = false;

    loop {
        let reserved = (if left_more { 2 } else { 0 }) + (if right_more { 2 } else { 0 });
        let total: u16 = widths[start..end].iter().sum();
        if total + reserved <= area_width || end - start <= 1 { break; }
        if app.active > start {
            start += 1;
            left_more = true;
        } else if end > app.active + 1 {
            end -= 1;
            right_more = true;
        } else {
            break;
        }
    }

    // A single tab wider than the area is clipped rather than clamped away.
    if end - start == 1 && widths[start] > area_width {
        widths[start] = area_width;
        left_more = false;
        right_more = false;
    }

    let mut x = 0u16;
    if left_more {
        out.push(TabLayout { kind: TabKind::LeftMore, x, width: 2 });
        x += 2;
    }
    for i in start..end {
        let width = widths[i].min(area_width.saturating_sub(x));
        if width == 0 { break; }
        out.push(TabLayout { kind: TabKind::Editor(i), x, width });
        x += width;
    }
    if right_more && x + 2 <= area_width {
        out.push(TabLayout { kind: TabKind::RightMore, x, width: 2 });
    }
    out
}

pub fn render(frame: &mut Frame, app: &App, tabs_area: Rect, crumbs_area: Rect) {
    render_tabs(frame, app, tabs_area);
    render_breadcrumbs(frame, app, crumbs_area);
}

fn render_tabs(frame: &mut Frame, app: &App, area: Rect) {
    if area.height == 0 || area.width == 0 { return; }
    let theme = &app.theme;
    frame.render_widget(Paragraph::new("").style(Style::default().bg(theme.tab_inactive_bg)), area);

    for tab in tab_layout(app, area.width) {
        let rect = Rect { x: area.x + tab.x, y: area.y, width: tab.width, height: 1 };
        match tab.kind {
            TabKind::LeftMore | TabKind::RightMore => {
                let symbol = if tab.kind == TabKind::LeftMore { "‹" } else { "›" };
                frame.render_widget(
                    Paragraph::new(Line::from(Span::styled(symbol, Style::default().fg(theme.tab_inactive_fg)))),
                    rect,
                );
            }
            TabKind::Editor(index) => {
                let editor = &app.editors[index];
                let active = index == app.active;
                let bg = if active { theme.tab_active_bg } else { theme.tab_inactive_bg };
                let fg = if active { theme.tab_active_fg } else { theme.tab_inactive_fg };
                let marker = if editor.dirty() { "●" } else { "×" };
                let marker_color = if editor.dirty() { theme.tab_dirty } else { theme.tab_inactive_fg };

                let icon = icon_span(app, editor.buffer.language, 0);
                let name_width = tab.width.saturating_sub(6) as usize;
                let name = truncate(&editor.name(), name_width);
                let used = 1 + 3 + name.chars().count() + 1;
                let padding = (tab.width as usize).saturating_sub(used + 1);

                let mut spans = vec![
                    Span::styled("▏".to_string(),
                        Style::default().fg(if active { theme.tab_active_border } else { theme.tab_border })),
                ];
                spans.push(icon);
                spans.push(Span::styled(name,
                    Style::default().fg(fg).add_modifier(if active { Modifier::BOLD } else { Modifier::empty() })));
                spans.push(Span::styled(" ".repeat(padding), Style::default()));
                spans.push(Span::styled(marker.to_string(), Style::default().fg(marker_color)));
                frame.render_widget(
                    Paragraph::new(Line::from(spans)).style(Style::default().bg(bg)),
                    rect,
                );
            }
        }
    }
}

fn render_breadcrumbs(frame: &mut Frame, app: &App, area: Rect) {
    if area.height == 0 || area.width == 0 { return; }
    let theme = &app.theme;
    let Some(editor) = app.active_editor() else { return; };
    let path = editor.path().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| editor.name());
    let parts: Vec<&str> = path.split(|c| c == '/' || c == '\\').filter(|s| !s.is_empty()).collect();

    let lang = editor.buffer.language;
    let icon = icon_span(app, lang, 0);
    let mut spans = vec![icon];
    let mut used = 3usize;
    for (i, part) in parts.iter().enumerate() {
        let sep = " › ";
        let piece = format!("{}{}", sep, part);
        let piece_len = piece.chars().count();
        if used + piece_len > area.width as usize { break; }
        spans.push(Span::styled(piece,
            Style::default().fg(if i + 1 == parts.len() { theme.side_bar_fg } else { theme.panel_inactive })));
        used += piece_len;
    }
    // Right-aligned file language, like VS Code's breadcrumb trailing details.
    let lang_label = format!("{} ", lang.name());
    if used + lang_label.chars().count() < area.width as usize {
        let pad = (area.width as usize).saturating_sub(used + lang_label.chars().count());
        spans.push(Span::styled(" ".repeat(pad), Style::default()));
        spans.push(Span::styled(lang_label, Style::default().fg(theme.panel_inactive)));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(theme.editor_bg)),
        area,
    );
}
