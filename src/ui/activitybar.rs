use crate::app::{App, SideBarView};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

const ICONS: [(&str, SideBarView, &str); 4] = [
    ("▤", SideBarView::Explorer, "Explorer"),
    ("⌕", SideBarView::Search, "Search"),
    ("⑂", SideBarView::SourceControl, "Source Control"),
    ("▷", SideBarView::RunDebug, "Run and Debug"),
];

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    if area.width == 0 || area.height == 0 { return; }
    let theme = &app.theme;
    frame.render_widget(Paragraph::new("").style(Style::default().bg(theme.activity_bar_bg)), area);

    let icon_x = area.x + 1;
    let icon_w = area.width.saturating_sub(1);

    for (i, (icon, view, _label)) in ICONS.iter().enumerate() {
        let y = area.y + 1 + i as u16 * 2;
        if y >= area.y + area.height.saturating_sub(4) { break; }
        let active = app.sidebar_visible && app.sidebar_view == *view;
        let fg = if active { theme.activity_bar_active_fg } else { theme.activity_bar_fg };

        // Active indicator stripe on the left edge.
        frame.render_widget(
            Paragraph::new(" ").style(Style::default().bg(
                if active { theme.activity_bar_active_fg } else { theme.activity_bar_bg })),
            Rect { x: area.x, y, width: 1, height: 1 },
        );
        let mut spans = vec![Span::styled(
            pad_icon(icon, icon_w as usize),
            Style::default().fg(fg).bg(theme.activity_bar_bg)
                .add_modifier(if active { Modifier::BOLD } else { Modifier::empty() }),
        )];

        // Git change badge on the Source Control icon.
        if *view == SideBarView::SourceControl && app.git_changes() > 0 && icon_w >= 2 {
            spans[0] = Span::styled(pad_icon(icon, 1), Style::default().fg(fg));
            spans.push(Span::styled(
                app.git_changes().to_string().chars().next().unwrap_or('9').to_string(),
                Style::default().fg(Color::White).bg(theme.activity_bar_badge).add_modifier(Modifier::BOLD),
            ));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)),
            Rect { x: icon_x, y, width: icon_w, height: 1 });
    }

    // Subtle separator above the account/settings cluster.
    let bottom = area.y + area.height.saturating_sub(1);
    if area.height > 6 {
        frame.render_widget(
            Paragraph::new("─".repeat(area.width as usize))
                .style(Style::default().fg(theme.activity_bar_border)),
            Rect { x: area.x, y: bottom.saturating_sub(3), width: area.width, height: 1 },
        );
    }

    let footer = [
        (bottom.saturating_sub(2), "◉"),
        (bottom, "⚙"),
    ];
    for (y, icon) in footer {
        if y >= area.y {
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(pad_icon(icon, icon_w as usize),
                    Style::default().fg(theme.activity_bar_fg).bg(theme.activity_bar_bg)))),
                Rect { x: icon_x, y, width: icon_w, height: 1 },
            );
        }
    }
}

fn pad_icon(icon: &str, width: usize) -> String {
    let len = icon.chars().count();
    if len >= width { icon.to_string() } else { format!("{}{}", icon, " ".repeat(width - len)) }
}
