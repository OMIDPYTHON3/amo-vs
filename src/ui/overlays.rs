use super::layout::Rects;
use super::sidebar::draw_field;
use super::style::truncate;
use crate::app::command::{menu_entries, MENU_NAMES};
use crate::app::{App, ContextMenuState, Overlay};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

fn centered(area: Rect, width: u16, height: u16, y_offset: u16) -> Rect {
    let w = width.min(area.width.saturating_sub(2)).max(20);
    let h = height.min(area.height.saturating_sub(2)).max(3);
    Rect {
        x: area.x + area.width.saturating_sub(w) / 2,
        y: area.y + y_offset.min(area.height.saturating_sub(h)),
        width: w,
        height: h,
    }
}

pub fn render(frame: &mut Frame, app: &App, area: Rect, rects: &Rects) {
    match app.overlay {
        Some(Overlay::CommandPalette) | Some(Overlay::QuickOpen) => render_palette(frame, app, area),
        Some(Overlay::Prompt) => render_prompt(frame, app, area),
        Some(Overlay::Confirm) => render_confirm(frame, app, area),
        Some(Overlay::Help) => render_help(frame, app, area),
        Some(Overlay::Find) | Some(Overlay::Replace) => render_find(frame, app, rects),
        Some(Overlay::GlobalSearch) => {}
        Some(Overlay::GotoLine) => {}
        Some(Overlay::Menu) => render_menu(frame, app, area),
        Some(Overlay::FileBrowser) => render_browser(frame, app, area),
        Some(Overlay::ContextMenu) => render_context_menu(frame, app, area),
        Some(Overlay::ThemePicker) => render_theme_picker(frame, app, area),
        None => {}
    }
}

fn render_theme_picker(frame: &mut Frame, app: &App, area: Rect) {
    let Some(picker) = &app.theme_picker else { return; };
    let theme = &app.theme; // live preview theme
    let rect = centered(area, 66, 20, 2);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Block::default().borders(Borders::ALL).border_style(Style::default().fg(theme.focus_border))
            .title(" Select Color Theme ").style(Style::default().bg(theme.widget_bg)),
        rect,
    );
    draw_field(frame, Rect { x: rect.x + 2, y: rect.y + 2, width: rect.width - 4, height: 3 },
        &picker.query, true, theme, "Type to filter themes");
    let list_y = rect.y + 6;
    let list_h = rect.height.saturating_sub(9) as usize;
    if picker.names.is_empty() {
        frame.render_widget(Paragraph::new(Line::from(Span::styled("  No matching themes",
            Style::default().fg(theme.panel_inactive)))),
            Rect { x: rect.x + 2, y: list_y, width: rect.width - 4, height: 1 });
    }
    for (i, name) in picker.names.iter().take(list_h).enumerate() {
        let selected = i == picker.selected;
        let bg = if selected { theme.list_active_bg } else { theme.widget_bg };
        let swatches: Vec<Span<'static>> = ["editor_bg", "side_bar_bg", "status_bar_bg", "syntax_keyword"]
            .iter()
            .filter_map(|key| crate::themes::builtin(name).map(|t| match *key {
                "editor_bg" => t.editor_bg,
                "side_bar_bg" => t.side_bar_bg,
                "status_bar_bg" => t.status_bar_bg,
                _ => t.syntax_keyword,
            }))
            .map(|c| Span::styled("  ".to_string(), Style::default().bg(c)))
            .collect();
        let mut spans = vec![
            Span::styled(if selected { "▏" } else { " " }.to_string(),
                Style::default().fg(if selected { theme.focus_border } else { theme.widget_bg })),
        ];
        spans.extend(swatches);
        spans.push(Span::styled(format!("  {}", name),
            Style::default().fg(if selected { theme.editor_fg } else { theme.side_bar_fg })
                .add_modifier(if selected { Modifier::BOLD } else { Modifier::empty() })));
        frame.render_widget(Paragraph::new(Line::from(spans)).style(Style::default().bg(bg)),
            Rect { x: rect.x + 1, y: list_y + i as u16, width: rect.width - 2, height: 1 });
    }
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled("  ↑↓ preview   Enter apply   Esc cancel",
            Style::default().fg(theme.panel_inactive)))),
        Rect { x: rect.x + 2, y: rect.y + rect.height - 2, width: rect.width - 4, height: 1 });
}

pub fn context_menu_rect(menu: &ContextMenuState, screen: Rect) -> Rect {
    let width = menu.actions.iter()
        .map(|a| a.label().len() + a.hint().len() + 6)
        .max().unwrap_or(18)
        .clamp(16, 34) as u16;
    let height = (menu.actions.len() + 2) as u16;
    let x = menu.x.min(screen.width.saturating_sub(width + 1));
    let y = menu.y.min(screen.height.saturating_sub(height + 1));
    Rect { x, y, width, height }
}

fn render_context_menu(frame: &mut Frame, app: &App, area: Rect) {
    let Some(menu) = &app.context_menu else { return; };
    let theme = &app.theme;
    let rect = context_menu_rect(menu, area);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Block::default().borders(Borders::ALL).border_style(Style::default().fg(theme.focus_border))
            .style(Style::default().bg(theme.widget_bg)),
        rect,
    );
    for (i, action) in menu.actions.iter().enumerate() {
        let y = rect.y + 1 + i as u16;
        let selected = menu.selected == i;
        let bg = if selected { theme.list_active_bg } else { theme.widget_bg };
        let label_w = rect.width as usize - action.hint().len() - 5;
        let line = Line::from(vec![
            Span::styled("  ".to_string(), Style::default()),
            Span::styled(truncate(action.label(), label_w),
                Style::default().fg(if selected { theme.editor_fg } else { theme.side_bar_fg })),
            Span::styled(" ".repeat(rect.width as usize - label_w - action.hint().len() - 3), Style::default()),
            Span::styled(action.hint().to_string(), Style::default().fg(theme.panel_inactive)),
        ]);
        frame.render_widget(Paragraph::new(line).style(Style::default().bg(bg)),
            Rect { x: rect.x, y, width: rect.width, height: 1 });
    }
}

fn render_palette(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let rect = centered(area, 82, 22, 2);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.widget_border))
            .style(Style::default().bg(theme.widget_bg)),
        rect,
    );
    let is_command = app.palette.query.trim_start().starts_with('>');
    let title = if is_command { " Command Palette " } else { " Go to File " };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(title, Style::default().fg(theme.side_bar_title)))),
        Rect { x: rect.x + 1, y: rect.y, width: rect.width - 2, height: 1 },
    );

    let input_y = rect.y + 1;
    let prefix = if is_command { "> " } else { "" };
    let input = format!("{}{}", prefix, app.palette.query.trim_start_matches('>'));
    let input_len = input.chars().count();
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(input, Style::default().fg(theme.editor_fg))))
            .style(Style::default().bg(theme.input_bg)),
        Rect { x: rect.x + 1, y: input_y, width: rect.width - 2, height: 1 },
    );
    let cursor_x = rect.x + 1 + (input_len as u16).min(rect.width.saturating_sub(2));
    frame.render_widget(
        Paragraph::new(" ").style(Style::default().bg(theme.cursor_bg)),
        Rect { x: cursor_x.saturating_sub(1), y: input_y, width: 1, height: 1 },
    );

    let list_y = rect.y + 3;
    let list_height = rect.height.saturating_sub(6) as usize;
    if app.palette.items.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled("  No matching results",
                Style::default().fg(theme.panel_inactive)))),
            Rect { x: rect.x + 1, y: list_y, width: rect.width - 2, height: 1 });
    }
    let scroll = app.palette.scroll.min(app.palette.items.len().saturating_sub(list_height));
    for (i, item) in app.palette.items.iter().skip(scroll).take(list_height).enumerate() {
        let absolute = i + scroll;
        let selected = absolute == app.palette.selected;
        let bg = if selected { theme.list_active_bg } else { theme.widget_bg };
        let fg = if selected { theme.editor_fg } else { theme.side_bar_fg };

        let label = highlight_matches(&item.label, &item.indices, theme, fg);
        let mut spans = vec![
            Span::styled(if selected { "▏" } else { " " }.to_string(),
                Style::default().fg(if selected { theme.focus_border } else { theme.widget_bg })),
            Span::styled("  ".to_string(), Style::default()),
        ];
        spans.extend(label);
        let used = item.label.chars().count() + 4;
        if rect.width as usize > used + 18 {
            spans.push(Span::styled(" ".repeat(rect.width as usize - used - 14), Style::default()));
            spans.push(Span::styled(truncate(&item.detail, 12), Style::default().fg(theme.panel_inactive)));
        }
        frame.render_widget(
            Paragraph::new(Line::from(spans)).style(Style::default().bg(bg)),
            Rect { x: rect.x + 1, y: list_y + i as u16, width: rect.width - 2, height: 1 },
        );
    }

    let count = format!(" {} result{}  ", app.palette.items.len(),
        if app.palette.items.len() == 1 { "" } else { "s" });
    let hints = "↑↓ navigate   Enter select   Esc dismiss";
    let gap = (rect.width as usize).saturating_sub(count.chars().count() + hints.chars().count() + 2);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(count, Style::default().fg(theme.focus_border)),
            Span::styled(" ".repeat(gap), Style::default()),
            Span::styled(hints.to_string(), Style::default().fg(theme.panel_inactive)),
        ])),
        Rect { x: rect.x + 1, y: rect.y + rect.height - 2, width: rect.width - 2, height: 1 },
    );
}

fn highlight_matches(label: &str, indices: &[usize], theme: &crate::themes::Theme, base_fg: ratatui::style::Color) -> Vec<Span<'static>> {
    if indices.is_empty() {
        return vec![Span::styled(label.to_string(), Style::default().fg(base_fg))];
    }
    let matched: Vec<usize> = indices.iter().copied().collect();
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut is_match = false;
    for (i, c) in label.chars().enumerate() {
        let m = matched.contains(&i);
        if current.is_empty() { is_match = m; }
        else if m != is_match {
            let style = if is_match {
                Style::default().fg(theme.syntax_variable).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(base_fg)
            };
            spans.push(Span::styled(std::mem::take(&mut current), style));
            is_match = m;
        }
        current.push(c);
    }
    if !current.is_empty() {
        let style = if is_match {
            Style::default().fg(theme.syntax_variable).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(base_fg)
        };
        spans.push(Span::styled(current, style));
    }
    spans
}

fn render_prompt(frame: &mut Frame, app: &App, area: Rect) {
    let Some(prompt) = &app.prompt else { return; };
    let theme = &app.theme;
    let rect = centered(area, 64, 7, (area.height / 3).saturating_sub(2));
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Block::default().borders(Borders::ALL).border_style(Style::default().fg(theme.focus_border))
            .title(format!(" {} ", prompt.title)).style(Style::default().bg(theme.widget_bg)),
        rect,
    );
    draw_field(frame, Rect { x: rect.x + 2, y: rect.y + 2, width: rect.width - 4, height: 3 },
        &prompt.value, true, theme, "");
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled("  Enter confirm   Esc cancel", Style::default().fg(theme.panel_inactive)))),
        Rect { x: rect.x + 2, y: rect.y + 5, width: rect.width - 4, height: 1 },
    );
}

fn render_confirm(frame: &mut Frame, app: &App, area: Rect) {
    let Some(confirm) = &app.confirm else { return; };
    let theme = &app.theme;
    let is_exit = confirm.style == crate::app::ConfirmStyle::Exit;
    let height = if is_exit { 10 } else { 8 };
    let rect = centered(area, 68, height, area.height.saturating_sub(height) / 3);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Block::default().borders(Borders::ALL).border_style(Style::default().fg(theme.focus_border))
            .title(if is_exit { " Exit AMO VS " } else { " Confirm " })
            .style(Style::default().bg(theme.widget_bg)),
        rect,
    );
    // Message, wrapped over two lines when necessary.
    let msg = truncate(&confirm.message, rect.width as usize - 4);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(msg, Style::default().fg(theme.editor_fg)))),
        Rect { x: rect.x + 2, y: rect.y + 2, width: rect.width - 4, height: 1 },
    );

    if is_exit {
        let dirty = app.editors.iter().filter(|e| e.dirty()).count();
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("  {} unsaved file(s) - choose an action", dirty),
                Style::default().fg(theme.warning),
            ))),
            Rect { x: rect.x + 2, y: rect.y + 3, width: rect.width - 4, height: 1 },
        );
        let options = [
            ("1", "Save All and Exit", theme.status_bar_bg),
            ("2", "Exit Without Saving", theme.widget_border),
            ("3", "Cancel", theme.widget_border),
        ];
        for (i, (key, label, accent)) in options.iter().enumerate() {
            let y = rect.y + 5 + i as u16;
            let selected = confirm.selected == i;
            let bg = if selected { theme.list_active_bg } else { theme.widget_bg };
            let line = Line::from(vec![
                Span::styled(format!("  [{}] ", key), Style::default().fg(*accent).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:<24}", label), Style::default().fg(if selected { theme.editor_fg } else { theme.side_bar_fg })),
                Span::styled(if selected { "◀" } else { " " }, Style::default().fg(theme.focus_border)),
            ]);
            frame.render_widget(Paragraph::new(line).style(Style::default().bg(bg)),
                Rect { x: rect.x + 1, y, width: rect.width - 2, height: 1 });
        }
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled("  ←→ select   Enter confirm   1/2/3 shortcut   Esc cancel",
                Style::default().fg(theme.panel_inactive)))),
            Rect { x: rect.x + 2, y: rect.y + rect.height - 2, width: rect.width - 4, height: 1 },
        );
    } else {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("  [Y]es", Style::default().fg(theme.status_bar_bg).add_modifier(Modifier::BOLD)),
                Span::styled("     ", Style::default()),
                Span::styled("[N]o / Esc", Style::default().fg(theme.side_bar_fg)),
            ])),
            Rect { x: rect.x + 2, y: rect.y + 4, width: rect.width - 4, height: 1 },
        );
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled("  Enter confirms the highlighted action",
                Style::default().fg(theme.panel_inactive)))),
            Rect { x: rect.x + 2, y: rect.y + 6, width: rect.width - 4, height: 1 },
        );
    }
}

fn render_help(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let rect = centered(area, 72, 26, 2);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Block::default().borders(Borders::ALL).border_style(Style::default().fg(theme.focus_border))
            .title(" AMO VS - Keyboard Shortcuts ").style(Style::default().bg(theme.widget_bg)),
        rect,
    );
    let shortcuts = [
        ("Ctrl+P", "Go to File"), ("Ctrl+Shift+P / F1", "Command Palette"),
        ("Ctrl+O / Ctrl+K Ctrl+O", "Open File / Folder"), ("Ctrl+S / Ctrl+Shift+S", "Save / Save As"),
        ("Ctrl+N / Ctrl+W", "New / Close Editor"), ("Ctrl+F / Ctrl+H", "Find / Replace"),
        ("Ctrl+Shift+F", "Find in Files"), ("Ctrl+G", "Go to Line"),
        ("Ctrl+/", "Toggle Line Comment"), ("Ctrl+Z / Ctrl+Y", "Undo / Redo"),
        ("Ctrl+B / Ctrl+J", "Toggle Side Bar / Panel"), ("Ctrl+` / Ctrl+Shift+`", "Toggle / New Terminal"),
        ("Ctrl+Tab", "Next Editor"), ("Alt+Z", "Toggle Word Wrap"),
        ("F5", "Run Active File"), ("F2 / Delete", "Rename / Delete (Explorer)"),
    ];
    for (i, (key, desc)) in shortcuts.iter().enumerate() {
        if i + 2 >= rect.height as usize { break; }
        let line = Line::from(vec![
            Span::styled(format!("  {:<26}", key), Style::default().fg(theme.syntax_variable)),
            Span::styled(desc.to_string(), Style::default().fg(theme.editor_fg)),
        ]);
        frame.render_widget(Paragraph::new(line), Rect { x: rect.x + 1, y: rect.y + 2 + i as u16, width: rect.width - 2, height: 1 });
    }
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled("  Press any key to close", Style::default().fg(theme.panel_inactive)))),
        Rect { x: rect.x + 2, y: rect.y + rect.height - 2, width: rect.width - 4, height: 1 },
    );
}

fn render_find(frame: &mut Frame, app: &App, rects: &Rects) {
    let theme = &app.theme;
    let editor = rects.editor;
    if editor.width < 28 { return; }
    let w = editor.width.min(62);
    let rect = Rect { x: editor.x + editor.width.saturating_sub(w + 2), y: editor.y, width: w, height: if app.overlay == Some(Overlay::Replace) { 3 } else { 2 } };
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Block::default().borders(Borders::ALL).border_style(Style::default().fg(theme.widget_border))
            .style(Style::default().bg(theme.widget_bg)),
        rect,
    );

    let count = if app.find.matches.is_empty() {
        "No results".to_string()
    } else {
        format!("{}/{}", app.find.current + 1, app.find.matches.len())
    };
    let count_color = if app.find.matches.is_empty() { theme.panel_inactive } else { theme.focus_border };
    let toggle = |label: &str, active: bool| Span::styled(
        format!(" {} ", label),
        if active {
            Style::default().fg(theme.editor_fg).bg(theme.list_active_bg).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.panel_inactive)
        },
    );

    let flags_w = 3 + 3 + count.chars().count() + 8;
    let shown = truncate(&app.find.query, (w as usize).saturating_sub(flags_w + 4));
    let used = 2 + shown.chars().count();
    let spaces = (w as usize).saturating_sub(used + flags_w);
    let search_line = Line::from(vec![
        Span::styled("  ".to_string(), Style::default()),
        Span::styled(shown, Style::default().fg(theme.editor_fg)),
        Span::styled(" ".repeat(spaces), Style::default()),
        toggle("Aa", app.find.case_sensitive),
        toggle("ab", app.find.whole_word),
        Span::styled(format!(" {} ", count), Style::default().fg(count_color)),
    ]);
    frame.render_widget(Paragraph::new(search_line), Rect { x: rect.x, y: rect.y, width: rect.width, height: 1 });

    if app.overlay == Some(Overlay::Replace) {
        let replace_line = Line::from(vec![
            Span::styled("  ".to_string(), Style::default()),
            Span::styled(truncate(&app.find.replacement, w.saturating_sub(32) as usize),
                Style::default().fg(theme.editor_fg)),
        ]);
        frame.render_widget(Paragraph::new(replace_line), Rect { x: rect.x, y: rect.y + 1, width: rect.width, height: 1 });
        let hint = if w >= 52 { "  Alt+R replace   Alt+A replace all   Tab field" } else { "  Alt+R / Alt+A" };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(hint, Style::default().fg(theme.panel_inactive)))),
            Rect { x: rect.x, y: rect.y + 2, width: rect.width, height: 1 },
        );
    }
}

fn render_menu(frame: &mut Frame, app: &App, area: Rect) {
    let Some(index) = app.menu.open else { return; };
    if index >= MENU_NAMES.len() { return; }
    let theme = &app.theme;
    let x = MENU_NAMES.iter().take(index).map(|n| n.len() + 4).sum::<usize>() as u16;
    let entries = menu_entries(index);
    let width = entries.iter().map(|e| e.label.len() + e.hint.len() + 6).max().unwrap_or(16).min(40) as u16;
    let height = entries.len().min(area.height.saturating_sub(2) as usize) as u16;
    let rect = Rect { x: x.min(area.width.saturating_sub(width)), y: 1, width, height };
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Block::default().borders(Borders::ALL).border_style(Style::default().fg(theme.widget_border))
            .style(Style::default().bg(theme.widget_bg)),
        rect,
    );
    for (i, entry) in entries.iter().enumerate().take(height as usize) {
        let y = rect.y + 1 + i as u16;
        if y >= rect.y + rect.height { break; }
        if entry.separator {
            frame.render_widget(Paragraph::new("─".repeat(rect.width as usize - 2)),
                Rect { x: rect.x + 1, y, width: rect.width - 2, height: 1 });
            continue;
        }
        let selected = i == app.menu.selected;
        let bg = if selected { theme.list_active_bg } else { theme.widget_bg };
        let label_w = rect.width as usize - entry.hint.len() - 6;
        let line = Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(truncate(entry.label, label_w), Style::default().fg(theme.editor_fg)),
            Span::styled(" ".repeat(rect.width as usize - label_w - entry.hint.len() - 4), Style::default()),
            Span::styled(entry.hint.to_string(), Style::default().fg(theme.panel_inactive)),
            Span::styled(" ", Style::default()),
        ]);
        frame.render_widget(Paragraph::new(line).style(Style::default().bg(bg)),
            Rect { x: rect.x, y, width: rect.width, height: 1 });
    }
}

fn render_browser(frame: &mut Frame, app: &App, area: Rect) {
    let Some(browser) = &app.file_browser else { return; };
    let theme = &app.theme;
    let rect = centered(area, 78, 22, 2);
    frame.render_widget(Clear, rect);
    let title = if browser.mode == crate::app::FileBrowserMode::OpenFolder { " Open Folder " } else { " Open File " };
    frame.render_widget(
        Block::default().borders(Borders::ALL).border_style(Style::default().fg(theme.focus_border))
            .title(title).style(Style::default().bg(theme.widget_bg)),
        rect,
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(format!("  {}", truncate(&browser.dir.display().to_string(), rect.width as usize - 4)),
            Style::default().fg(theme.syntax_variable)))),
        Rect { x: rect.x + 1, y: rect.y + 1, width: rect.width - 2, height: 1 },
    );
    draw_field(frame, Rect { x: rect.x + 2, y: rect.y + 2, width: rect.width - 4, height: 3 },
        &browser.filter, true, theme, "Filter");
    let list_y = rect.y + 6;
    let list_h = rect.height.saturating_sub(9) as usize;
    for (i, (path, name, is_dir)) in browser.entries.iter().enumerate().take(list_h) {
        let selected = i == browser.selected;
        let bg = if selected { theme.list_active_bg } else { theme.widget_bg };
        let icon = super::icons::icon_for_path(app, path, *is_dir, false);
        let suffix = if *is_dir { "/" } else { "" };
        let line = Line::from(vec![
            Span::styled("  ".to_string(), Style::default()),
            icon,
            Span::styled(format!(" {}{}", name, suffix),
                Style::default().fg(if *is_dir { theme.syntax_variable } else { theme.editor_fg })),
        ]);
        frame.render_widget(Paragraph::new(line).style(Style::default().bg(bg)),
            Rect { x: rect.x + 1, y: list_y + i as u16, width: rect.width - 2, height: 1 });
    }
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            if browser.mode == crate::app::FileBrowserMode::OpenFolder {
                "  ↑↓ select   Enter open   Ctrl+O open this folder   Esc cancel"
            } else {
                "  ↑↓ select   Enter open   Backspace up   Esc cancel"
            },
            Style::default().fg(theme.panel_inactive)))),
        Rect { x: rect.x + 1, y: rect.y + rect.height - 2, width: rect.width - 2, height: 1 },
    );
}
