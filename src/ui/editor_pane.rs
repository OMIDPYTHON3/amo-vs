use crate::app::App;
use crate::editor::{bracket, Span};
use crate::themes::SyntaxRole;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span as TSpan};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

struct RenderRow {
    line: Option<usize>,
    cells: Vec<(char, Style)>,
}

pub fn render(frame: &mut Frame, app: &mut App, gutter: Rect, text_area: Rect, minimap: Rect) {
    frame.render_widget(Paragraph::new("").style(Style::default().bg(app.theme.editor_bg)), gutter);
    frame.render_widget(Paragraph::new("").style(Style::default().bg(app.theme.editor_bg)), text_area);
    if minimap.width > 0 {
        frame.render_widget(Paragraph::new("").style(Style::default().bg(app.theme.editor_bg)), minimap);
    }

    let width = text_area.width as usize;
    let height = text_area.height as usize;

    // The Welcome tab is a dedicated rendered page, not editable buffer text.
    if app.active_editor().map(|e| e.is_welcome).unwrap_or(false) {
        crate::ui::welcome::render(frame, app, text_area);
        return;
    }

    let config = app.config.clone();

    let (spans, cursor_line, cursor_col, matched, selection, scroll_row, scroll_col) = {
        let Some(editor) = app.active_editor_mut() else { return; };
        let buffer = &editor.buffer;
        let view = &editor.view;
        let spans = editor.highlighter.spans(buffer, buffer.revision()).to_vec();
        let cursor_line = buffer.char_to_line(view.cursor);
        let cursor_col = view.cursor.saturating_sub(buffer.line_to_char(cursor_line));
        let matched = if config.bracket_matching {
            bracket::find_match(buffer, view.cursor, &config)
        } else { None };
        (spans, cursor_line, cursor_col, matched, view.selection(), view.scroll_row, view.scroll_col)
    };

    let rows = build_rows(app, &spans, width, height, cursor_line, cursor_col, matched, selection);

    for (row_index, render_row) in rows.iter().enumerate() {
        if row_index >= height { break; }
        let y = text_area.y + row_index as u16;

        // Gutter: line number for the first physical row of a source line.
        if gutter.width > 0 {
            let number = render_row.line.map(|l| (l + 1).to_string()).unwrap_or_default();
            let pad = (gutter.width as usize).saturating_sub(2);
            let label = format!("{:>width$} ", number, width = pad);
            let color = if render_row.line == Some(cursor_line) {
                app.theme.line_number_active_fg
            } else {
                app.theme.line_number_fg
            };
            frame.render_widget(
                Paragraph::new(Line::from(TSpan::styled(label, Style::default().fg(color))))
                    .style(Style::default().bg(app.theme.editor_bg)),
                Rect { x: gutter.x, y, width: gutter.width, height: 1 },
            );
        }

        let line = Line::from(
            render_row.cells
                .iter()
                .map(|(c, s)| TSpan::styled(c.to_string(), *s))
                .collect::<Vec<_>>(),
        );
        frame.render_widget(Paragraph::new(line).style(Style::default().bg(app.theme.editor_bg)),
            Rect { x: text_area.x, y, width: text_area.width, height: 1 });

    }

    if minimap.width > 0 && minimap.height > 0 {
        crate::ui::minimap::render(frame, app, &spans, minimap, scroll_row, height);
    }

    // Cursor line highlight in the gutter (subtle vertical marker).
    let cursor_screen_row = cursor_line.saturating_sub(scroll_row);
    if app.config.highlight_current_line && cursor_screen_row < height && gutter.width > 0 {
        frame.render_widget(
            Paragraph::new(" ").style(Style::default().fg(app.theme.indent_guide)),
            Rect { x: gutter.x + gutter.width - 1, y: text_area.y + cursor_screen_row as u16, width: 1, height: 1 },
        );
    }

    if app.completion.visible && app.overlay.is_none() && app.focus == crate::app::Focus::Editor {
        crate::ui::completion::render(frame, app, text_area, cursor_screen_row, cursor_col, scroll_col);
    }
}

#[allow(clippy::too_many_arguments)]
fn build_rows(
    app: &App,
    spans: &[Vec<Span>],
    width: usize,
    height: usize,
    cursor_line: usize,
    cursor_col: usize,
    matched: Option<usize>,
    selection: Option<(usize, usize)>,
) -> Vec<RenderRow> {
    let theme = &app.theme;
    let editor = app.active_editor().unwrap();
    let buffer = &editor.buffer;
    let view = &editor.view;
    let mut rows = Vec::new();

    for line in view.scroll_row..buffer.len_lines() {
        if rows.len() >= height { break; }
        let text = buffer.line_text(line);
        let line_start = buffer.line_to_char(line);
        let line_len = buffer.line_content_len(line);
        let line_end = line_start + line_len;

        let roles = char_roles(&text, spans.get(line).map(|s| s.as_slice()).unwrap_or(&[]));
        let chars: Vec<char> = text.chars().collect();

        let sel = selection.and_then(|(s, e)| {
            if e <= line_start || s >= line_end { return None; }
            let a = s.saturating_sub(line_start).min(line_len);
            let b = e.saturating_sub(line_start).min(line_len);
            if a < b { Some((a, b)) } else { None }
        });

        let match_col = matched.and_then(|m| {
            if m >= line_start && m < line_end { Some(m - line_start) } else { None }
        });

        let mut cells: Vec<(char, Style)> = Vec::with_capacity(chars.len());
        for (i, &ch) in chars.iter().enumerate() {
            let mut style = theme.style(*roles.get(i).unwrap_or(&SyntaxRole::Plain));
            if app.config.highlight_current_line && line == cursor_line {
                style = style.bg(theme.current_line_bg);
            }
            if sel.map(|(a, b)| i >= a && i < b).unwrap_or(false) {
                style = style.bg(theme.selection_bg);
            }
            if match_col == Some(i) {
                style = style.bg(theme.bracket_match_bg)
                    .fg(theme.bracket_match_border)
                    .add_modifier(Modifier::BOLD);
            }
            if i == cursor_col && line == cursor_line && app.focus == crate::app::Focus::Editor
                && app.overlay.is_none()
            {
                style = Style::default().fg(theme.editor_bg).bg(theme.cursor_bg)
                    .add_modifier(Modifier::BOLD);
            }
            if app.config.show_whitespace && ch == ' ' {
                cells.push(('·', style.fg(theme.whitespace)));
            } else if app.config.show_whitespace && ch == '\t' {
                cells.push(('→', style.fg(theme.whitespace)));
            } else {
                cells.push((ch, style));
            }
        }
        if line == cursor_line && cursor_col >= chars.len() && app.focus == crate::app::Focus::Editor && app.overlay.is_none() {
            cells.push((' ', Style::default().fg(theme.editor_bg).bg(theme.cursor_bg)));
        }

        if app.config.word_wrap && width > 0 {
            let mut chunk: Vec<(char, Style)> = Vec::new();
            let mut chunk_width = 0usize;
            let mut first = true;
            let mut pushed = false;
            for cell in cells {
                let cw = unicode_width::UnicodeWidthChar::width(cell.0).unwrap_or(1);
                if chunk_width + cw > width && !chunk.is_empty() {
                    rows.push(RenderRow { line: if first { Some(line) } else { None }, cells: std::mem::take(&mut chunk) });
                    first = false;
                    pushed = true;
                    chunk_width = 0;
                    if rows.len() >= height { break; }
                }
                chunk.push(cell);
                chunk_width += cw;
            }
            if rows.len() < height && (!chunk.is_empty() || !pushed) {
                rows.push(RenderRow { line: if first { Some(line) } else { None }, cells: chunk });
            }
        } else {
            let start = view.scroll_col.min(cells.len());
            let end = (start + width).min(cells.len());
            rows.push(RenderRow { line: Some(line), cells: cells[start..end].to_vec() });
        }
    }

    if rows.is_empty() {
        rows.push(RenderRow { line: None, cells: Vec::new() });
    }
    rows
}

fn char_roles(text: &str, ranges: &[Span]) -> Vec<SyntaxRole> {
    let mut byte_roles = vec![SyntaxRole::Plain; text.len()];
    for &(start, end, role) in ranges {
        for b in start.min(text.len())..end.min(text.len()) {
            byte_roles[b] = role;
        }
    }
    let mut out = Vec::new();
    for (i, _) in text.char_indices() {
        out.push(*byte_roles.get(i).unwrap_or(&SyntaxRole::Plain));
    }
    out
}


