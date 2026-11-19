use super::style::truncate;
use crate::app::{App, Focus, Overlay, SideBarView};
use crate::themes::Theme;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

#[derive(Debug, Clone)]
pub enum ExplorerRow {
    Editor(usize),
    Tree(crate::workspace::FlatEntry),
}

/// Rows displayed in the explorer, shared by rendering and mouse handling.
pub fn explorer_rows(app: &App) -> Vec<ExplorerRow> {
    let mut rows = Vec::new();
    for i in 0..app.editors.len() {
        rows.push(ExplorerRow::Editor(i));
    }
    if let Some(workspace) = &app.workspace {
        for entry in workspace.flatten() {
            rows.push(ExplorerRow::Tree(entry));
        }
    }
    rows
}

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    if area.width == 0 || area.height == 0 { return; }
    match app.sidebar_view {
        SideBarView::Explorer => render_explorer(frame, app, area),
        SideBarView::Search => render_search(frame, app, area),
        SideBarView::SourceControl => render_git(frame, app, area),
        SideBarView::RunDebug => render_run(frame, app, area),
    }
}

fn section_header(theme: &Theme, title: &str, width: usize) -> Line<'static> {
    let label = format!(" ▾ {}", title.to_uppercase());
    let used = label.chars().count();
    let mut spans = vec![Span::styled(
        label,
        Style::default().fg(theme.side_bar_fg).add_modifier(Modifier::BOLD),
    )];
    if width > used + 2 {
        spans.push(Span::styled(" ".repeat(width - used - 2), Style::default()));
        spans.push(Span::styled("⋯", Style::default().fg(theme.side_bar_fg)));
    }
    Line::from(spans)
}

fn render_explorer(frame: &mut Frame, app: &App, area: Rect) {
    use super::icons::{icon_for_path, icon_span};

    let theme = &app.theme;
    frame.render_widget(Paragraph::new("").style(Style::default().bg(theme.side_bar_bg)), area);
    let mut y = area.y;

    // Title strip with subtle action buttons on the right.
    let title_w = area.width as usize;
    let mut title_spans = vec![Span::styled(" EXPLORER",
        Style::default().fg(theme.side_bar_title).add_modifier(Modifier::BOLD))];
    if title_w > 16 {
        title_spans.push(Span::styled(" ".repeat(title_w - 14), Style::default()));
        title_spans.push(Span::styled("⋯ ", Style::default().fg(theme.side_bar_fg)));
    }
    frame.render_widget(Paragraph::new(Line::from(title_spans)),
        Rect { x: area.x, y, width: area.width, height: 1 });
    y += 2;

    let rows = explorer_rows(app);
    let active_path = app.active_editor().and_then(|e| e.path()).map(|p| p.to_path_buf());
    let visible = area.height.saturating_sub((y - area.y) + 2) as usize;
    let scroll = app.explorer_scroll.min(rows.len().saturating_sub(visible));

    // ---- Open Editors ----------------------------------------------------
    let editor_count = app.editors.len();
    if editor_count > 0 && y < area.y + area.height {
        frame.render_widget(Paragraph::new(section_header(theme, "Open Editors", area.width as usize)),
            Rect { x: area.x, y, width: area.width, height: 1 });
        y += 1;
        for i in 0..editor_count {
            if y >= area.y + area.height { break; }
            let editor = &app.editors[i];
            let selected = app.focus == Focus::Explorer && app.explorer_selected == i;
            let is_active = i == app.active;
            let bg = if selected { theme.side_bar_active_bg }
                else if is_active { theme.side_bar_hover_bg }
                else { theme.side_bar_bg };
            let fg = if is_active { theme.side_bar_title } else { theme.side_bar_fg };
            let mut spans = vec![
                Span::styled("  ".to_string(), Style::default()),
                icon_span(app, editor.buffer.language, 0),
            ];
            let used = 4usize;
            let name_max = (area.width as usize).saturating_sub(used + 7);
            spans.push(Span::styled(truncate(&editor.name(), name_max),
                Style::default().fg(fg).add_modifier(if is_active { Modifier::BOLD } else { Modifier::empty() })));
            let used = used + editor.name().chars().count().min(name_max) + 1;
            spans.push(Span::styled(" ".repeat((area.width as usize).saturating_sub(used + 3)), Style::default()));
            if editor.dirty() {
                spans.push(Span::styled("● ".to_string(), Style::default().fg(theme.tab_dirty)));
            } else {
                spans.push(Span::styled("× ".to_string(), Style::default().fg(theme.panel_inactive)));
            }
            frame.render_widget(Paragraph::new(Line::from(spans)).style(Style::default().bg(bg)),
                Rect { x: area.x, y, width: area.width, height: 1 });
            y += 1;
        }
        y += 1;
    }

    // ---- Folder section --------------------------------------------------
    if y < area.y + area.height {
        let root_name = app.workspace.as_ref()
            .map(|w| w.root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| w.root.display().to_string()))
            .unwrap_or_else(|| "NO FOLDER OPENED".into());
        frame.render_widget(Paragraph::new(section_header(theme, &root_name, area.width as usize)),
            Rect { x: area.x, y, width: area.width, height: 1 });
        y += 1;
    }

    for row_index in scroll..rows.len() {
        if y >= area.y + area.height { break; }
        let ExplorerRow::Tree(entry) = &rows[row_index] else { continue; };

        let focused = app.focus == Focus::Explorer && app.explorer_selected == row_index;
        let is_active = active_path.as_deref() == Some(entry.path.as_path());
        let bg = if focused { theme.side_bar_active_bg }
            else if is_active { theme.side_bar_hover_bg }
            else { theme.side_bar_bg };

        // Tree guides: "│ " for continuing ancestors, "  " for last ones.
        let mut prefix = String::new();
        for &ancestor_last in &entry.guides {
            prefix.push_str(if ancestor_last { "  " } else { "│ " });
        }
        prefix.push_str(if entry.last { "└─" } else { "├─" });

        let arrow = if entry.is_dir {
            if entry.expanded { "▾" } else { "▸" }
        } else { " " };
        let icon = icon_for_path(app, &entry.path, entry.is_dir, entry.expanded);
        let decoration = if entry.git == crate::workspace::GitState::Clean {
            String::new()
        } else {
            format!(" {}", crate::services::git::short_label(entry.git))
        };

        let prefix_w = prefix.chars().count() + 2; // arrow + space
        let name_max = (area.width as usize)
            .saturating_sub(prefix_w + 2 + decoration.chars().count() + 1);
        let name_style = Style::default().fg(if entry.is_dir { theme.side_bar_title } else { theme.side_bar_fg })
            .add_modifier(if is_active { Modifier::BOLD } else { Modifier::empty() });

        let mut spans = vec![
            Span::styled(prefix, Style::default().fg(theme.tree_guide)),
            Span::styled(format!("{} ", arrow), Style::default().fg(theme.side_bar_fg)),
            icon,
            Span::styled(truncate(&entry.name, name_max), name_style),
        ];
        let used = prefix_w + 2 + entry.name.chars().count().min(name_max) + decoration.chars().count();
        spans.push(Span::styled(" ".repeat((area.width as usize).saturating_sub(used)), Style::default()));
        if !decoration.is_empty() {
            spans.push(Span::styled(decoration, theme.git_style(entry.git)));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)).style(Style::default().bg(bg)),
            Rect { x: area.x, y, width: area.width, height: 1 });
        y += 1;
    }
}

fn render_search(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    frame.render_widget(Paragraph::new("").style(Style::default().bg(theme.side_bar_bg)), area);
    let mut y = area.y;
    frame.render_widget(Paragraph::new(Line::from(Span::styled(" SEARCH",
        Style::default().fg(theme.side_bar_title).add_modifier(Modifier::BOLD)))),
        Rect { x: area.x, y, width: area.width, height: 1 });
    y += 2;

    let input_w = area.width.saturating_sub(2);
    draw_input(frame, Rect { x: area.x + 1, y, width: input_w, height: 1 },
        &app.search.query, app.overlay == Some(Overlay::GlobalSearch) && app.search.field == 0, theme, "Search");
    y += 2;
    if app.search.show_replace {
        draw_input(frame, Rect { x: area.x + 1, y, width: input_w, height: 1 },
            &app.search.replacement, app.overlay == Some(Overlay::GlobalSearch) && app.search.field == 1, theme, "Replace");
        y += 2;
    }

    let flags = format!(" {} Aa  {} ab  {}",
        if app.search.case_sensitive { "[x]" } else { "[ ]" },
        if app.search.whole_word { "[x]" } else { "[ ]" },
        if app.search.show_replace { "Replace" } else { "" });
    frame.render_widget(Paragraph::new(Line::from(Span::styled(flags, Style::default().fg(theme.panel_inactive)))),
        Rect { x: area.x + 1, y, width: area.width.saturating_sub(1), height: 1 });
    y += 2;

    let count = app.search.results.len();
    let mut files: Vec<_> = app.search.results.iter().map(|r| &r.path).collect();
    files.sort();
    files.dedup();
    frame.render_widget(Paragraph::new(Line::from(Span::styled(
        format!(" {} result{} in {} file{}", count, if count == 1 { "" } else { "s" },
            files.len(), if files.len() == 1 { "" } else { "s" }),
        Style::default().fg(theme.side_bar_title).add_modifier(Modifier::BOLD)))),
        Rect { x: area.x + 1, y, width: area.width.saturating_sub(1), height: 1 });
    y += 1;

    let row_limit = area.height.saturating_sub(y - area.y) as usize;
    for (i, result) in app.search.results.iter().take(row_limit).enumerate() {
        let selected = app.search.selected == i;
        let bg = if selected { theme.list_active_bg } else { theme.side_bar_bg };
        let name = result.path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let mut spans = vec![
            super::icons::icon_span(app, crate::editor::Language::detect(&result.path), 0),
            Span::styled(truncate(&name, 10), Style::default().fg(theme.side_bar_title)),
            Span::styled(format!(":{}  ", result.line), Style::default().fg(theme.panel_inactive)),
        ];
        let used = 2 + name.chars().count().min(10) + result.line.to_string().len() + 3;
        let text = truncate(result.text.trim(), area.width.saturating_sub(used as u16 + 1) as usize);
        spans.push(Span::styled(text, Style::default().fg(if selected { theme.editor_fg } else { theme.side_bar_fg })));
        frame.render_widget(Paragraph::new(Line::from(spans)).style(Style::default().bg(bg)),
            Rect { x: area.x, y, width: area.width, height: 1 });
        y += 1;
    }
}

fn render_git(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    frame.render_widget(Paragraph::new("").style(Style::default().bg(theme.side_bar_bg)), area);
    let mut y = area.y;
    frame.render_widget(Paragraph::new(Line::from(Span::styled(" SOURCE CONTROL",
        Style::default().fg(theme.side_bar_title).add_modifier(Modifier::BOLD)))),
        Rect { x: area.x, y, width: area.width, height: 1 });
    y += 2;

    let branch = app.workspace.as_ref().and_then(|w| w.git.branch.clone()).unwrap_or_else(|| "none".into());
    frame.render_widget(Paragraph::new(Line::from(Span::styled(
        truncate(&format!(" ⑂ {}", branch), area.width as usize),
        Style::default().fg(theme.side_bar_fg)))),
        Rect { x: area.x, y, width: area.width, height: 1 });
    y += 2;

    let changes: Vec<_> = app.workspace.as_ref()
        .map(|w| w.git.changes.iter().collect())
        .unwrap_or_default();
    frame.render_widget(Paragraph::new(section_header(theme, &format!("Changes ({})", changes.len()), area.width as usize)),
        Rect { x: area.x, y, width: area.width, height: 1 });
    y += 1;

    let mut changes: Vec<_> = changes;
    changes.sort_by_key(|(p, _)| (*p).clone());
    for (path, state) in changes {
        if y >= area.y + area.height { break; }
        let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let label = crate::services::git::short_label(*state);
        let line = Line::from(vec![
            Span::styled(format!("  {} ", label), theme.git_style(*state)),
            Span::styled(truncate(&name, area.width.saturating_sub(5) as usize), Style::default().fg(theme.side_bar_fg)),
        ]);
        frame.render_widget(Paragraph::new(line).style(Style::default().bg(theme.side_bar_bg)),
            Rect { x: area.x, y, width: area.width, height: 1 });
        y += 1;
    }
}

fn render_run(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    frame.render_widget(Paragraph::new("").style(Style::default().bg(theme.side_bar_bg)), area);
    let mut y = area.y;
    frame.render_widget(Paragraph::new(Line::from(Span::styled(" RUN AND DEBUG",
        Style::default().fg(theme.side_bar_title).add_modifier(Modifier::BOLD)))),
        Rect { x: area.x, y, width: area.width, height: 1 });
    y += 3;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.widget_border))
        .title(" Run ");
    let inner = block.inner(Rect { x: area.x + 1, y, width: area.width.saturating_sub(2), height: 5 });
    frame.render_widget(block, Rect { x: area.x + 1, y, width: area.width.saturating_sub(2), height: 5 });
    let lines = vec![
        Line::from(Span::styled(" ▷ Run Active File", Style::default().fg(theme.editor_fg))),
        Line::from(Span::styled("   F5 / Ctrl+F5", Style::default().fg(theme.panel_inactive))),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}

/// A bordered single-line input used by prompts and dialogs.
pub fn draw_field(frame: &mut Frame, area: Rect, value: &str, focused: bool, theme: &Theme, placeholder: &str) {
    if area.width < 4 || area.height < 1 { return; }
    let block = Block::default().borders(Borders::ALL)
        .border_style(Style::default().fg(if focused { theme.input_border } else { theme.widget_border }))
        .style(Style::default().bg(theme.input_bg));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let text = if value.is_empty() { placeholder } else { value };
    let style = if value.is_empty() {
        Style::default().fg(theme.panel_inactive).bg(theme.input_bg)
    } else {
        Style::default().fg(theme.editor_fg).bg(theme.input_bg)
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(truncate(text, inner.width as usize), style))),
        inner,
    );
    if focused {
        let cursor_x = inner.x + (value.chars().count() as u16).min(inner.width.saturating_sub(1));
        frame.render_widget(Paragraph::new(" ").style(Style::default().bg(theme.cursor_bg)),
            Rect { x: cursor_x, y: inner.y, width: 1, height: 1 });
    }
}

pub fn draw_input(frame: &mut Frame, area: Rect, value: &str, focused: bool, theme: &Theme, placeholder: &str) {
    let text = if value.is_empty() { placeholder } else { value };
    let style = if value.is_empty() {
        Style::default().fg(theme.panel_inactive).bg(theme.input_bg)
    } else {
        Style::default().fg(theme.editor_fg).bg(theme.input_bg)
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(truncate(text, area.width as usize), style)))
            .style(Style::default().bg(theme.input_bg)),
        area,
    );
    if focused {
        frame.render_widget(
            Paragraph::new(" ").style(Style::default().bg(theme.focus_border)),
            Rect { x: area.x + (value.chars().count() as u16).min(area.width.saturating_sub(1)), y: area.y, width: 1, height: 1 },
        );
    }
}
