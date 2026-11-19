use crate::editor::buffer::Buffer;

/// Cursor, selection and scroll state for one editor tab.
#[derive(Debug, Clone)]
pub struct EditorView {
    /// Cursor position as an absolute character index.
    pub cursor: usize,
    /// Selection anchor; `Some` means a selection is active.
    pub anchor: Option<usize>,
    /// Column preserved when moving vertically across lines.
    pub desired_col: usize,
    /// First visible line.
    pub scroll_row: usize,
    /// First visible column (horizontal scroll).
    pub scroll_col: usize,
}

impl Default for EditorView {
    fn default() -> Self {
        Self { cursor: 0, anchor: None, desired_col: 0, scroll_row: 0, scroll_col: 0 }
    }
}

impl EditorView {
    pub fn new(cursor: usize) -> Self {
        Self { cursor, desired_col: 0, ..Default::default() }
    }

    fn line_col(&self, buffer: &Buffer, idx: usize) -> (usize, usize) {
        let line = buffer.char_to_line(idx.min(buffer.len_chars()));
        let start = buffer.line_to_char(line);
        (line, idx.saturating_sub(start))
    }

    fn begin_move(&mut self, extend: bool) {
        if extend && self.anchor.is_none() {
            self.anchor = Some(self.cursor);
        }
    }

    fn finish_move(&mut self, buffer: &Buffer, extend: bool) {
        if !extend {
            self.anchor = None;
        }
        let (_, col) = self.line_col(buffer, self.cursor);
        self.desired_col = col;
    }

    pub fn move_left(&mut self, buffer: &Buffer, extend: bool) {
        self.begin_move(extend);
        self.cursor = self.cursor.saturating_sub(1);
        self.finish_move(buffer, extend);
    }

    pub fn move_right(&mut self, buffer: &Buffer, extend: bool) {
        self.begin_move(extend);
        self.cursor = (self.cursor + 1).min(buffer.len_chars());
        self.finish_move(buffer, extend);
    }

    pub fn move_up(&mut self, buffer: &Buffer, extend: bool) {
        self.begin_move(extend);
        let (line, _) = self.line_col(buffer, self.cursor);
        if line > 0 {
            let target = line - 1;
            let col = self.desired_col.min(buffer.line_content_len(target));
            self.cursor = buffer.line_to_char(target) + col;
        }
        if !extend { self.anchor = None; }
    }

    pub fn move_down(&mut self, buffer: &Buffer, extend: bool) {
        self.begin_move(extend);
        let (line, _) = self.line_col(buffer, self.cursor);
        if line + 1 < buffer.len_lines() {
            let target = line + 1;
            let col = self.desired_col.min(buffer.line_content_len(target));
            self.cursor = buffer.line_to_char(target) + col;
        }
        if !extend { self.anchor = None; }
    }

    pub fn move_home(&mut self, buffer: &Buffer, extend: bool) {
        self.begin_move(extend);
        let (line, col) = self.line_col(buffer, self.cursor);
        let start = buffer.line_to_char(line);
        let text = buffer.line_text(line);
        let first_non_ws = text.chars().take_while(|c| c.is_whitespace()).count();
        let target = if col > first_non_ws { first_non_ws } else { 0 };
        self.cursor = start + target;
        self.finish_move(buffer, extend);
    }

    pub fn move_end(&mut self, buffer: &Buffer, extend: bool) {
        self.begin_move(extend);
        let (line, _) = self.line_col(buffer, self.cursor);
        self.cursor = buffer.line_to_char(line) + buffer.line_content_len(line);
        self.finish_move(buffer, extend);
    }

    pub fn move_doc_start(&mut self, extend: bool) {
        self.begin_move(extend);
        self.cursor = 0;
        self.desired_col = 0;
        if !extend { self.anchor = None; }
    }

    pub fn move_doc_end(&mut self, buffer: &Buffer, extend: bool) {
        self.begin_move(extend);
        self.cursor = buffer.len_chars();
        self.desired_col = 0;
        if !extend { self.anchor = None; }
    }

    pub fn move_page_up(&mut self, buffer: &Buffer, rows: usize, extend: bool) {
        self.begin_move(extend);
        let (line, _) = self.line_col(buffer, self.cursor);
        let target = line.saturating_sub(rows.max(1));
        let col = self.desired_col.min(buffer.line_content_len(target));
        self.cursor = buffer.line_to_char(target) + col;
        self.scroll_row = self.scroll_row.saturating_sub(rows.max(1));
        if !extend { self.anchor = None; }
    }

    pub fn move_page_down(&mut self, buffer: &Buffer, rows: usize, extend: bool) {
        self.begin_move(extend);
        let (line, _) = self.line_col(buffer, self.cursor);
        let target = (line + rows.max(1)).min(buffer.len_lines().saturating_sub(1));
        let col = self.desired_col.min(buffer.line_content_len(target));
        self.cursor = buffer.line_to_char(target) + col;
        if !extend { self.anchor = None; }
    }

    pub fn move_word_left(&mut self, buffer: &Buffer, extend: bool) {
        self.begin_move(extend);
        let mut idx = self.cursor;
        let is_word = |c: char| c.is_alphanumeric() || c == '_';
        while idx > 0 {
            let prev = buffer.char_at(idx - 1).unwrap_or(' ');
            if is_word(prev) { break; }
            idx -= 1;
        }
        while idx > 0 {
            let prev = buffer.char_at(idx - 1).unwrap_or(' ');
            if !is_word(prev) { break; }
            idx -= 1;
        }
        self.cursor = idx;
        self.finish_move(buffer, extend);
    }

    pub fn move_word_right(&mut self, buffer: &Buffer, extend: bool) {
        self.begin_move(extend);
        let mut idx = self.cursor;
        let len = buffer.len_chars();
        let is_word = |c: char| c.is_alphanumeric() || c == '_';
        while idx < len {
            let c = buffer.char_at(idx).unwrap_or(' ');
            if is_word(c) { break; }
            idx += 1;
        }
        while idx < len {
            let c = buffer.char_at(idx).unwrap_or(' ');
            if !is_word(c) { break; }
            idx += 1;
        }
        self.cursor = idx;
        self.finish_move(buffer, extend);
    }

    pub fn set_cursor(&mut self, buffer: &Buffer, idx: usize, extend: bool) {
        self.begin_move(extend);
        self.cursor = idx.min(buffer.len_chars());
        self.finish_move(buffer, extend);
    }

    pub fn selection(&self) -> Option<(usize, usize)> {
        self.anchor.map(|a| (a.min(self.cursor), a.max(self.cursor)))
    }

    pub fn has_selection(&self) -> bool {
        self.anchor.map(|a| a != self.cursor).unwrap_or(false)
    }

    pub fn select_all(&mut self, buffer: &Buffer) {
        self.anchor = Some(0);
        self.cursor = buffer.len_chars();
        self.desired_col = 0;
    }

    pub fn clear_selection(&mut self) { self.anchor = None; }

    /// Adjust scrolling so the cursor remains visible.
    pub fn ensure_visible(&mut self, buffer: &Buffer, rows: usize, cols: usize) {
        let (line, col) = self.line_col(buffer, self.cursor);
        if line < self.scroll_row {
            self.scroll_row = line;
        } else if rows > 0 && line >= self.scroll_row + rows {
            self.scroll_row = line - rows + 1;
        }
        let margin = 4usize;
        if col < self.scroll_col {
            self.scroll_col = col;
        } else if cols > 0 && col >= self.scroll_col + cols.saturating_sub(margin) {
            self.scroll_col = col.saturating_sub(cols.saturating_sub(margin).max(1)) + 1;
        }
    }

    pub fn line(&self, buffer: &Buffer) -> usize {
        buffer.char_to_line(self.cursor.min(buffer.len_chars()))
    }

    pub fn column(&self, buffer: &Buffer) -> usize {
        let (_, col) = self.line_col(buffer, self.cursor);
        col
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::Language;

    fn view(text: &str) -> (EditorView, Buffer) {
        (EditorView::new(0), Buffer::from_text(text, Language::PlainText))
    }

    #[test]
    fn vertical_movement_preserves_desired_column() {
        let (mut v, b) = view("abcd\nxy\nlonger line");
        v.set_cursor(&b, 3, false);
        v.move_down(&b, false);
        assert_eq!(v.cursor, b.line_to_char(1) + 2);
        v.move_down(&b, false);
        assert_eq!(v.cursor, b.line_to_char(2) + 3);
    }

    #[test]
    fn selection_range() {
        let (mut v, b) = view("hello world");
        v.set_cursor(&b, 3, false);
        v.move_right(&b, true);
        v.move_right(&b, true);
        assert_eq!(v.selection(), Some((3, 5)));
    }
}
