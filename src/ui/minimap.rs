use crate::app::App;
use crate::themes::SyntaxRole;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Pure geometry: map a source line to a minimap row.
pub fn line_to_row(line: usize, total_lines: usize, rows: usize) -> usize {
    if total_lines == 0 || rows == 0 { return 0; }
    (line * rows / total_lines).min(rows - 1)
}

/// Pure geometry: map a minimap row back to a source line (used for clicks).
pub fn row_to_line(row: usize, total_lines: usize, rows: usize) -> usize {
    if rows == 0 || total_lines == 0 { return 0; }
    (row * total_lines / rows).min(total_lines.saturating_sub(1))
}

/// The half-open band of minimap rows covered by the current viewport.
pub fn viewport_band(scroll_row: usize, visible_rows: usize, total_lines: usize, rows: usize) -> (usize, usize) {
    if rows == 0 { return (0, 0); }
    let start = line_to_row(scroll_row, total_lines, rows);
    let end = line_to_row(scroll_row + visible_rows, total_lines, rows).max(start + 1).min(rows);
    (start, end)
}

/// Render a real miniature of the document. Each minimap row summarises a band
/// of source lines; columns sample syntax roles from a representative line.
pub fn render(
    frame: &mut Frame,
    app: &App,
    spans: &[Vec<(usize, usize, SyntaxRole)>],
    area: Rect,
    scroll_row: usize,
    visible_rows: usize,
) {
    if area.width == 0 || area.height == 0 { return; }
    let theme = &app.theme;
    let Some(editor) = app.active_editor() else { return; };
    let buffer = &editor.buffer;
    let total_lines = buffer.len_lines().max(1);
    let rows = area.height as usize;
    let cols = area.width as usize;
    let (vp_start, vp_end) = viewport_band(scroll_row, visible_rows, total_lines, rows);

    for row in 0..rows {
        let band_start = row * total_lines / rows;
        let band_end = ((row + 1) * total_lines / rows).max(band_start + 1).min(total_lines);
        let in_viewport = row >= vp_start && row < vp_end;
        let bg = if in_viewport { theme.minimap_viewport_bg } else { theme.minimap_bg };

        // Pick the most informative sampled line in this band.
        let mut sample = band_start;
        let mut best_len = 0usize;
        let sample_count = (band_end - band_start).min(4);
        for offset in 0..sample_count {
            let line = band_start + offset;
            let len = buffer.line_content_len(line);
            if len > best_len { best_len = len; sample = line; }
        }

        let mut cells = vec![(' ', Style::default().fg(theme.minimap_bg).bg(bg)); cols];
        if best_len > 0 {
            let text = buffer.line_text(sample);
            let roles = char_roles(&text, spans.get(sample).map(|s| s.as_slice()).unwrap_or(&[]));
            let chars: Vec<char> = text.chars().collect();
            if !chars.is_empty() {
                let chars_per_col = (chars.len() + cols - 1) / cols;
                for col in 0..cols {
                    let char_index = col * chars_per_col;
                    if char_index >= chars.len() { break; }
                    let role = roles.get(char_index).copied().unwrap_or(SyntaxRole::Plain);
                    let mut color = theme.style(role).fg.unwrap_or(theme.line_number_fg);
                    if role == SyntaxRole::Plain {
                        // VS Code renders minimap code at reduced opacity; a
                        // muted line-number gray is the terminal equivalent.
                        color = theme.line_number_fg;
                    }
                    let mut style = Style::default().fg(color).bg(bg);
                    if in_viewport { style = style.add_modifier(Modifier::BOLD); }
                    cells[col] = ('▬', style);
                }
            }
        }

        // Left slider rail: a small marker inside the viewport band.
        if cols >= 2 {
            let marker = if in_viewport { '▐' } else { ' ' };
            let mcolor = if in_viewport { theme.minimap_slider } else { bg };
            cells[0] = (marker, Style::default().fg(mcolor).bg(bg));
        }

        let line = Line::from(cells.into_iter()
            .map(|(c, s)| Span::styled(c.to_string(), s))
            .collect::<Vec<_>>());
        frame.render_widget(Paragraph::new(line).style(Style::default().bg(theme.minimap_bg)),
            Rect { x: area.x, y: area.y + row as u16, width: area.width, height: 1 });
    }
}

fn char_roles(text: &str, ranges: &[(usize, usize, SyntaxRole)]) -> Vec<SyntaxRole> {
    let mut byte_roles = vec![SyntaxRole::Plain; text.len()];
    for &(start, end, role) in ranges {
        for b in start.min(text.len())..end.min(text.len()) {
            byte_roles[b] = role;
        }
    }
    text.char_indices()
        .map(|(i, _)| byte_roles.get(i).copied().unwrap_or(SyntaxRole::Plain))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_and_row_roundtrip() {
        for (lines, rows) in [(10usize, 5usize), (100, 12), (3, 10), (1, 1)] {
            for line in 0..lines {
                let row = line_to_row(line, lines, rows);
                let back = row_to_line(row, lines, rows);
                assert!(back < lines);
            }
        }
    }

    #[test]
    fn viewport_band_follows_scroll() {
        let (start, end) = viewport_band(0, 10, 100, 20);
        assert_eq!(start, 0);
        assert_eq!(end, 2);
        let (start2, end2) = viewport_band(50, 10, 100, 20);
        assert_eq!(start2, 10);
        assert_eq!(end2, 12);
    }
}
