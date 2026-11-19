pub mod activitybar;
pub mod completion;
pub mod editor_pane;
pub mod icons;
pub mod layout;
pub mod menu;
pub mod minimap;
pub mod overlays;
pub mod panel;
pub mod sidebar;
pub mod statusbar;
pub mod style;
pub mod tabs;
pub mod welcome;

use crate::app::App;
use ratatui::layout::Rect;
use ratatui::style::{Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.size();
    let r = layout::compute(app, area);

    menu::render(frame, app, r.menu);
    activitybar::render(frame, app, r.activity);
    if r.sidebar.width > 0 {
        sidebar::render(frame, app, r.sidebar);
    }
    tabs::render(frame, app, r.tabs, r.breadcrumbs);
    editor_pane::render(frame, app, r.gutter, r.editor_text, r.minimap);
    if r.panel.height > 0 {
        panel::render(frame, app, r.panel_tabs, r.panel_body);
    }
    statusbar::render(frame, app, r.status);

    overlays::render(frame, app, area, &r);
    render_messages(frame, app, area, r.status.y);
}

fn render_messages(frame: &mut Frame, app: &App, area: Rect, status_y: u16) {
    if app.messages.is_empty() { return; }
    let msg = app.messages.last().unwrap();
    let color = match msg.kind {
        crate::app::MessageKind::Info => app.theme.status_bar_bg,
        crate::app::MessageKind::Warning => app.theme.warning,
        crate::app::MessageKind::Error => app.theme.error,
    };
    let width = (msg.text.chars().count() + 4).min(area.width as usize).min(72);
    let y = status_y.saturating_sub(1);
    let rect = Rect {
        x: area.x + area.width.saturating_sub(width as u16 + 1),
        y,
        width: width as u16,
        height: 1,
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            format!(" {} ", crate::ui::style::truncate(&msg.text, width - 2)),
            Style::default().fg(ratatui::style::Color::White).bg(color),
        )])).wrap(Wrap { trim: false }),
        rect,
    );
}
