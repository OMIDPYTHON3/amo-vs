use super::style::truncate;
use crate::app::App;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

/// Geometry shared by the renderer and mouse hit-testing.
pub fn popup_rect(
    app: &App,
    area: Rect,
    cursor_row: usize,
    cursor_col: usize,
    scroll_col: usize,
) -> Rect {
    if app.completion.items.is_empty() { return Rect { x: 0, y: 0, width: 0, height: 0 }; }
    let width = (area.width.saturating_sub(2)).min(54).max(18);
    // Two border rows plus a footer row.
    let height = ((app.completion.items.len() + 3).min(11).max(4)) as u16;
    let cursor_x = cursor_col.saturating_sub(scroll_col) as u16;
    let x = area.x + cursor_x.saturating_add(1).min(area.width.saturating_sub(width));
    let preferred = area.y + cursor_row as u16 + 1;
    let y = if preferred + height > area.y + area.height {
        area.y + (cursor_row as u16).saturating_sub(height).min(area.height.saturating_sub(height))
    } else {
        preferred
    };
    Rect { x, y, width, height }
}

pub fn render(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    cursor_row: usize,
    cursor_col: usize,
    scroll_col: usize,
) {
    if area.width < 24 || area.height < 6 { return; }
    let rect = popup_rect(app, area, cursor_row, cursor_col, scroll_col);
    if rect.width == 0 || rect.height == 0 { return; }
    let theme = &app.theme;
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Block::default().borders(Borders::ALL)
            .border_style(Style::default().fg(theme.focus_border))
            .title(" Suggestions ")
            .style(Style::default().bg(theme.widget_bg)),
        rect,
    );

    let list_height = rect.height.saturating_sub(3) as usize;
    for (i, item) in app.completion.items.iter().take(list_height).enumerate() {
        let selected = i == app.completion.selected;
        let bg = if selected { theme.list_active_bg } else { theme.widget_bg };
        let prefix = truncate(&app.completion.prefix, 12);
        let label = if selected || prefix.is_empty() {
            item.label.clone()
        } else {
            item.label.clone()
        };
        let detail = truncate(&item.detail, 18);
        let label_w = (rect.width as usize).saturating_sub(detail.chars().count() + 9);
        let shown = truncate(&label, label_w);
        let used = shown.chars().count() + detail.chars().count();
        let pad = (rect.width as usize).saturating_sub(used + 6);
        let spans = vec![
            Span::styled(if selected { "▏" } else { " " }.to_string(),
                Style::default().fg(if selected { theme.focus_border } else { theme.widget_bg })),
            Span::styled(format!("{} ", item.kind.icon()), Style::default().fg(theme.syntax_type)),
            Span::styled(shown, Style::default()
                .fg(if selected { theme.editor_fg } else { theme.side_bar_fg })
                .add_modifier(if selected { Modifier::BOLD } else { Modifier::empty() })),
            Span::styled(" ".repeat(pad), Style::default()),
            Span::styled(detail, Style::default().fg(theme.panel_inactive)),
        ];
        frame.render_widget(Paragraph::new(Line::from(spans)).style(Style::default().bg(bg)),
            Rect { x: rect.x, y: rect.y + 1 + i as u16, width: rect.width, height: 1 });
    }
    let hint = if rect.width >= 36 { "↑↓ navigate  Tab accept  Esc dismiss" } else { "Tab accept  Esc dismiss" };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(format!(" {}", hint),
            Style::default().fg(theme.panel_inactive)))),
        Rect { x: rect.x, y: rect.y + rect.height - 2, width: rect.width, height: 1 });
}
