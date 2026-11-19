use crate::app::App;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    if area.height == 0 || area.width == 0 { return; }
    let theme = &app.theme;
    let base = Style::default().fg(theme.status_bar_fg).bg(theme.status_bar_bg);
    let accent = Style::default().fg(theme.status_bar_accent_fg).bg(theme.status_bar_accent_bg);
    let width = area.width as usize;
    let compact = width < 100;
    let micro = width < 72;

    let mut left: Vec<Span<'static>> = vec![Span::styled("  ><  ".to_string(), accent)];

    if let Some(workspace) = &app.workspace {
        let branch = workspace.git.branch.clone().unwrap_or_else(|| "no-git".into());
        let label = if micro {
            format!(" {} ", truncate_chars(&branch, 10))
        } else {
            format!(" ⑂ {} ", truncate_chars(&branch, 24))
        };
        left.push(Span::styled(label, base));
        if !micro && (workspace.git.ahead > 0 || workspace.git.behind > 0) {
            left.push(Span::styled(format!(" ↑{} ↓{} ", workspace.git.ahead, workspace.git.behind), base));
        }
        let change_count = app.git_changes();
        if change_count > 0 {
            left.push(Span::styled(format!(" ◉ {} ", change_count), base));
        }
    }

    let (errors, warnings) = app.active_diagnostic_count();
    if errors > 0 {
        left.push(Span::styled(format!(" ✘ {} ", errors),
            Style::default().fg(theme.status_bar_fg).bg(theme.status_bar_error_bg)));
    }
    if warnings > 0 {
        left.push(Span::styled(format!(" ⚠ {} ", warnings),
            Style::default().fg(theme.status_bar_fg).bg(theme.status_bar_warning_bg)));
    }

    let mut right: Vec<Span<'static>> = Vec::new();
    if let Some(editor) = app.active_editor() {
        let line = editor.view.line(&editor.buffer) + 1;
        let col = editor.view.column(&editor.buffer) + 1;
        right.push(Span::styled(format!(" Ln {}, Col {}  ", line, col), base));
        if !compact {
            right.push(Span::styled(
                format!(" Spaces: {}  ", if app.config.insert_spaces { app.config.tab_size } else { 1 }), base));
        }
        if !micro {
            right.push(Span::styled(" UTF-8  ", base));
            right.push(Span::styled(" LF  ", base));
        }
        let lang = editor.buffer.language.name();
        right.push(Span::styled(format!(" {}  ", if micro { truncate_chars(lang, 8) } else { lang.to_string() }), base));
    } else {
        right.push(Span::styled(" AMO VS  ", base));
    }
    if !compact { right.push(Span::styled(" ◍ ", base)); }

    let text_width = |spans: &[Span<'static>]| -> usize {
        spans.iter().map(|s| UnicodeWidthStr::width(s.content.as_ref())).sum()
    };
    let left_w = text_width(&left);
    let right_w = text_width(&right);
    let gap = width.saturating_sub(left_w + right_w);
    let mut spans = left;
    spans.push(Span::styled(" ".repeat(gap), base));
    spans.extend(right);

    // If content still overflows, rely on Paragraph clipping to avoid wrapping.
    frame.render_widget(Paragraph::new(Line::from(spans)).style(base), area);
}

fn truncate_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max { return text.to_string(); }
    format!("{}…", text.chars().take(max.saturating_sub(1)).collect::<String>())
}
