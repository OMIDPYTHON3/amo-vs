use crate::app::{App, PanelTab};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub fn render(frame: &mut Frame, app: &App, tabs_area: Rect, body_area: Rect) {
    if tabs_area.height == 0 || body_area.height == 0 { return; }
    let theme = &app.theme;
    frame.render_widget(Paragraph::new("").style(Style::default().bg(theme.panel_bg)), tabs_area);

    let tabs = PanelTab::all();
    let mut x = tabs_area.x + 1;
    for (i, tab) in tabs.iter().enumerate() {
        let label = if tabs_area.width < 70 {
            match tab {
                PanelTab::Problems => "PROBLEMS",
                PanelTab::Output => "OUTPUT",
                PanelTab::DebugConsole => "DEBUG",
                PanelTab::Terminal => "TERMINAL",
            }
        } else { tab.label() };
        let w = label.chars().count() as u16 + 3;
        if x + w > tabs_area.x + tabs_area.width { break; }
        let active = app.panel.tab == *tab;
        let fg = if active { theme.panel_active } else { theme.panel_inactive };
        let mut spans = vec![
            Span::styled(if active { "▏" } else { " " }.to_string(),
                Style::default().fg(if active { theme.focus_border } else { theme.panel_border })),
            Span::styled(format!("{} ", label),
                Style::default().fg(fg).add_modifier(if active { Modifier::BOLD } else { Modifier::empty() })),
        ];
        if i + 1 < tabs.len() && x + w + 1 < tabs_area.x + tabs_area.width {
            spans.push(Span::styled("│ ".to_string(), Style::default().fg(theme.panel_border)));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)),
            Rect { x, y: tabs_area.y, width: w + 1, height: 1 });
        x += w;
    }

    // Terminal session tabs on the right.
    if app.panel.tab == PanelTab::Terminal && !app.terminal.sessions.is_empty() {
        let mut labels = String::new();
        for (i, session) in app.terminal.sessions.iter().enumerate() {
            if i > 0 { labels.push_str("  "); }
            let marker = if i == app.terminal.active { "● " } else { "  " };
            labels.push_str(&format!("{}{}", marker, session.name));
        }
        let start = tabs_area.x + tabs_area.width.saturating_sub(labels.chars().count() as u16 + 1);
        if start > x {
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(labels, Style::default().fg(theme.panel_inactive)))),
                Rect { x: start, y: tabs_area.y, width: tabs_area.width.saturating_sub(start - tabs_area.x), height: 1 },
            );
        }
    }

    frame.render_widget(Paragraph::new("").style(Style::default().bg(theme.panel_bg)), body_area);
    match app.panel.tab {
        PanelTab::Terminal => render_terminal(frame, app, body_area),
        PanelTab::Problems => render_problems(frame, app, body_area),
        PanelTab::Output => render_output(frame, app, body_area),
        PanelTab::DebugConsole => render_debug(frame, app, body_area),
    }
}

fn render_terminal(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let Some(session) = app.terminal.active() else {
        let lines = vec![
            Line::from(Span::styled("  No terminal sessions", Style::default().fg(theme.side_bar_title))),
            Line::from(Span::styled("  Press Ctrl+Shift+` or run \"Terminal: New Terminal\" from the Command Palette.", Style::default().fg(theme.panel_inactive))),
        ];
        frame.render_widget(Paragraph::new(lines), area);
        return;
    };

    let grid = session.screen.lines();
    let rows = area.height as usize;
    let offset = grid.len().saturating_sub(rows);
    for row in 0..rows {
        let Some(cells) = grid.get(offset + row) else { break; };
        let spans: Vec<Span> = cells.iter().map(|(c, style)| Span::styled(c.to_string(), *style)).collect();
        frame.render_widget(
            Paragraph::new(Line::from(spans)).style(Style::default().bg(theme.editor_bg)),
            Rect { x: area.x, y: area.y + row as u16, width: area.width, height: 1 },
        );
    }
}

fn render_problems(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    if app.diagnostics.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled("  No problems have been detected in the workspace.", Style::default().fg(theme.panel_inactive)))),
            area,
        );
        return;
    }
    let scroll = app.panel.scroll.min(app.diagnostics.len().saturating_sub(area.height as usize));
    for (i, diag) in app.diagnostics.iter().skip(scroll).take(area.height as usize).enumerate() {
        let color = match diag.severity {
            crate::app::Severity::Error => theme.error,
            crate::app::Severity::Warning => theme.warning,
            crate::app::Severity::Info => theme.info,
        };
        let icon = match diag.severity {
            crate::app::Severity::Error => "✘",
            crate::app::Severity::Warning => "⚠",
            crate::app::Severity::Info => "ℹ",
        };
        let name = diag.path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let text = format!(" {}  {}:{}:{}  {}", icon, name, diag.line, diag.col, diag.message);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(text, Style::default().fg(color)))),
            Rect { x: area.x, y: area.y + i as u16, width: area.width, height: 1 },
        );
    }
}

fn render_output(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let scroll = app.panel.scroll.min(app.output.len().saturating_sub(area.height as usize));
    for (i, line) in app.output.iter().skip(scroll).take(area.height as usize).enumerate() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(line.clone(), Style::default().fg(theme.panel_fg)))),
            Rect { x: area.x, y: area.y + i as u16, width: area.width, height: 1 },
        );
    }
}

fn render_debug(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let lines = vec![
        Line::from(Span::styled("  Debug Console", Style::default().fg(theme.side_bar_title).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled("  Run the active file with F5 to see its output in the terminal.", Style::default().fg(theme.panel_inactive))),
    ];
    frame.render_widget(Paragraph::new(lines), area);
}
