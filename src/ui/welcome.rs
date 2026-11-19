use super::style::truncate;
use crate::app::{App, WelcomeAction};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

/// Map a screen row to a welcome action (kept in sync with `render`).
pub fn hit_test(app: &App, area: Rect, row: u16) -> Option<usize> {
    for i in 0..3u16 {
        if row == area.y + 9 + i { return Some(i as usize); }
    }
    if !app.recent_projects.is_empty() {
        for i in 0..app.recent_projects.len().min(5) as u16 {
            if row == area.y + 14 + i { return Some(3 + i as usize); }
        }
    }
    None
}

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    if area.width < 24 || area.height < 6 { return; }
    let theme = &app.theme;
    let width = (area.width as usize).min(82);
    let x = area.x + (area.width as usize - width) as u16 / 2;
    let mut y = area.y + 1;
    let line = |frame: &mut Frame, spans: Vec<Span<'static>>, y: u16| {
        frame.render_widget(Paragraph::new(Line::from(spans)),
            Rect { x, y, width: width as u16, height: 1 });
    };

    // ---- Branding --------------------------------------------------------
    let brand = " █████  ██████   ██████   ██████ ";
    let brand2 = "██   ██ ██   ██ ██    ██ ██    ██";
    let brand3 = "███████ ██████  ██    ██ ██    ██";
    let brand4 = "██   ██ ██   ██ ██    ██ ██    ██";
    for (i, text) in [brand, brand2, brand3, brand4].iter().enumerate() {
        if y + i as u16 >= area.y + area.height { break; }
        line(frame, vec![Span::styled(text.to_string(), Style::default()
            .fg(theme.status_bar_bg).add_modifier(Modifier::BOLD))], y + i as u16);
    }
    y += 5;
    line(frame, vec![Span::styled("  AMO VS — Visual Studio Code, in your terminal", Style::default()
        .fg(theme.side_bar_title))], y);
    y += 2;

    let actions = app.welcome_actions();
    // ---- Quick actions ---------------------------------------------------
    if y < area.y + area.height {
        line(frame, vec![Span::styled(" Start", Style::default()
            .fg(theme.side_bar_title).add_modifier(Modifier::BOLD))], y);
        y += 1;
    }
    for (i, action) in actions.iter().take(3).enumerate() {
        if y >= area.y + area.height { break; }
        let selected = app.welcome_selected == i;
        let (label, hint) = match action {
            WelcomeAction::NewFile => ("New File", "Ctrl+N"),
            WelcomeAction::OpenFile => ("Open File", "Ctrl+O"),
            WelcomeAction::OpenFolder => ("Open Folder", "Ctrl+K Ctrl+O"),
            _ => ("", ""),
        };
        let mut spans = vec![
            Span::styled(if selected { " ▸ " } else { "   " }.to_string(),
                Style::default().fg(theme.focus_border)),
            Span::styled(format!("[{}] ", label), Style::default()
                .fg(if selected { theme.status_bar_bg } else { theme.side_bar_fg })
                .add_modifier(if selected { Modifier::BOLD } else { Modifier::empty() })),
        ];
        let used = 5 + label.chars().count() + 3;
        spans.push(Span::styled(" ".repeat(width.saturating_sub(used + hint.len() + 2)), Style::default()));
        spans.push(Span::styled(hint.to_string(), Style::default().fg(theme.panel_inactive)));
        line(frame, spans, y);
        y += 1;
    }
    y += 1;

    // ---- Recent projects -------------------------------------------------
    if y < area.y + area.height && !app.recent_projects.is_empty() {
        line(frame, vec![Span::styled(" Recent Projects", Style::default()
            .fg(theme.side_bar_title).add_modifier(Modifier::BOLD))], y);
        y += 1;
        for (i, path) in app.recent_projects.iter().take(5).enumerate() {
            if y >= area.y + area.height { break; }
            let action_index = 3 + i;
            let selected = app.welcome_selected == action_index;
            let name = path.file_name().map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| path.display().to_string());
            let parent = path.parent().map(|p| p.display().to_string()).unwrap_or_default();
            let label = format!("{} — {}", name, parent);
            let mut spans = vec![
                Span::styled(if selected { " ▸ " } else { "   " }.to_string(),
                    Style::default().fg(theme.focus_border)),
                super::icons::icon_for_path(app, path, true, false),
                Span::styled(truncate(&label, width.saturating_sub(12)),
                    Style::default().fg(if selected { theme.editor_fg } else { theme.side_bar_fg })),
            ];
            let used = 3 + 2 + label.chars().count().min(width.saturating_sub(12));
            spans.push(Span::styled(" ".repeat(width.saturating_sub(used + 2)), Style::default()));
            line(frame, spans, y);
            y += 1;
        }
        y += 1;
    }

    // ---- Keyboard shortcuts ---------------------------------------------
    if y + 4 < area.y + area.height {
        line(frame, vec![Span::styled(" Keyboard Shortcuts", Style::default()
            .fg(theme.side_bar_title).add_modifier(Modifier::BOLD))], y);
        y += 1;
        let shortcuts = [
            ("Ctrl+P", "Go to File"), ("F1", "Command Palette"),
            ("Ctrl+F", "Find"), ("Ctrl+Shift+F", "Find in Files"),
            ("Ctrl+B", "Toggle Side Bar"), ("Ctrl+`", "Toggle Terminal"),
            ("Ctrl+Esc", "Exit"), ("F5", "Run Active File"),
        ];
        let half = width / 2;
        for pair in shortcuts.chunks(2) {
            if y >= area.y + area.height { break; }
            let mut spans = Vec::new();
            for (i, (key, desc)) in pair.iter().enumerate() {
                let _col = i * half;
                let entry = format!("  {:<14}{}", key, desc);
                let pad = half.saturating_sub(UnicodeWidthStr::width(entry.as_str()));
                spans.push(Span::styled(entry, Style::default().fg(theme.side_bar_fg)));
                spans.push(Span::styled(" ".repeat(pad), Style::default()));
            }
            line(frame, spans, y);
            y += 1;
        }
    }

    if y < area.y + area.height {
        line(frame, vec![Span::styled("  ↑↓ select   Enter open   Ctrl+P go to file", Style::default()
            .fg(theme.panel_inactive))], y);
    }
}
