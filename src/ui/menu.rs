use super::style::truncate;
use crate::app::command::MENU_NAMES;
use crate::app::App;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    if area.height == 0 { return; }
    let theme = &app.theme;
    let mut content = String::new();
    for (i, name) in MENU_NAMES.iter().enumerate() {
        if i > 0 { content.push_str("  "); }
        content.push_str(&format!(" {} ", name));
    }
    let line = ratatui::text::Line::from(vec![
        ratatui::text::Span::styled(
            truncate(&content, area.width as usize),
            Style::default().fg(theme.menu_fg).bg(theme.menu_bg),
        ),
    ]);
    frame.render_widget(Paragraph::new(line).style(Style::default().bg(theme.menu_bg)), area);

    // Highlight the currently active (hovered/open) menu item.
    if let Some(active) = app.menu.open.or(if app.overlay == Some(crate::app::Overlay::Menu) { Some(app.menu.selected) } else { None }) {
        if active < MENU_NAMES.len() {
            let x = MENU_NAMES.iter().take(active).map(|n| n.len() + 4).sum::<usize>() as u16;
            let w = (MENU_NAMES[active].len() + 2) as u16;
            if x + w <= area.width {
                frame.render_widget(
                    Paragraph::new(ratatui::text::Line::from(ratatui::text::Span::styled(
                        format!(" {} ", MENU_NAMES[active]),
                        Style::default().fg(theme.menu_active_fg).bg(theme.menu_active_bg).add_modifier(Modifier::BOLD),
                    ))),
                    Rect { x: area.x + x, y: area.y, width: w, height: 1 },
                );
            }
        }
    }
}
