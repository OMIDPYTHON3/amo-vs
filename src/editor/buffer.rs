use crate::editor::Language;
use ropey::Rope;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Clone)]
enum EditOp {
    Insert { at: usize, text: String },
    Delete { at: usize, len: usize, text: String },
}

/// A UTF-8 text buffer backed by a rope, with grouped undo/redo and search.
/// All positions are character (not byte) indices, which keeps cursor math
/// correct for multi-byte Unicode text.
pub struct Buffer {
    rope: Rope,
    path: Option<PathBuf>,
    pub language: Language,
    pub dirty: bool,
    undo: Vec<EditOp>,
    redo: Vec<EditOp>,
    revision: u64,
    last_edit: Option<Instant>,
    last_insert_end: Option<usize>,
    last_delete_at: Option<usize>,
}

const COALESCE_WINDOW_MS: u128 = 600;

impl Buffer {
    pub fn empty() -> Self {
        Self {
            rope: Rope::new(),
            path: None,
            language: Language::PlainText,
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            revision: 0,
            last_edit: None,
            last_insert_end: None,
            last_delete_at: None,
        }
    }

    pub fn from_text(text: &str, language: Language) -> Self {
        let mut b = Self::empty();
        b.rope = Rope::from_str(text);
        b.language = language;
        b
    }

    pub fn from_file(path: &Path) -> io::Result<Self> {
        let bytes = std::fs::read(path)?;
        let text = String::from_utf8_lossy(&bytes);
        let language = Language::detect(path);
        let mut b = Self::from_text(&text, language);
        b.path = Some(path.to_path_buf());
        Ok(b)
    }

    // ---- Accessors -------------------------------------------------------

    pub fn path(&self) -> Option<&Path> { self.path.as_deref() }

    pub fn set_path(&mut self, path: PathBuf) {
        self.language = Language::detect(&path);
        self.path = Some(path);
    }

    pub fn name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled".into())
    }

    pub fn title(&self) -> String {
        self.path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled".into())
    }

    pub fn text(&self) -> String { self.rope.to_string() }

    pub fn slice(&self, start: usize, end: usize) -> String {
        if start >= end || start >= self.len_chars() {
            return String::new();
        }
        let end = end.min(self.len_chars());
        self.rope.slice(start..end).to_string()
    }

    pub fn len_chars(&self) -> usize { self.rope.len_chars() }
    pub fn len_lines(&self) -> usize { self.rope.len_lines() }
    pub fn len_bytes(&self) -> usize { self.rope.len_bytes() }
    pub fn revision(&self) -> u64 { self.revision }

    pub fn char_to_byte(&self, idx: usize) -> usize {
        self.rope.char_to_byte(idx.min(self.len_chars()))
    }

    pub fn char_to_line(&self, idx: usize) -> usize {
        self.rope.char_to_line(idx.min(self.len_chars()))
    }

    pub fn line_to_char(&self, line: usize) -> usize {
        self.rope.line_to_char(line.min(self.len_lines().saturating_sub(1)))
    }

    pub fn line_len_chars(&self, line: usize) -> usize {
        if line >= self.len_lines() { return 0; }
        self.rope.line(line).len_chars()
    }

    /// Length of the line excluding the trailing newline character(s).
    pub fn line_content_len(&self, line: usize) -> usize {
        if line >= self.len_lines() { return 0; }
        let slice = self.rope.line(line);
        let mut len = slice.len_chars();
        while len > 0 && matches!(slice.char(len - 1), '\n' | '\r') {
            len -= 1;
        }
        len
    }

    pub fn line_text(&self, line: usize) -> String {
        if line >= self.len_lines() { return String::new(); }
        let slice = self.rope.line(line);
        let mut end = slice.len_chars();
        while end > 0 && matches!(slice.char(end - 1), '\n' | '\r') {
            end -= 1;
        }
        slice.slice(..end).to_string()
    }

    pub fn char_at(&self, idx: usize) -> Option<char> {
        if idx < self.len_chars() { Some(self.rope.char(idx)) } else { None }
    }

    // ---- Editing ---------------------------------------------------------

    fn note_edit(&mut self) {
        self.dirty = true;
        self.revision = self.revision.wrapping_add(1);
        self.last_edit = Some(Instant::now());
    }

    fn can_coalesce(&self) -> bool {
        self.last_edit
            .map(|t| t.elapsed().as_millis() <= COALESCE_WINDOW_MS)
            .unwrap_or(false)
    }

    /// Insert `text` at character index `at`. `coalesce` groups rapid single
    /// character typing into one undo step.
    pub fn insert(&mut self, at: usize, text: &str, coalesce: bool) -> usize {
        if text.is_empty() { return at; }
        let at = at.min(self.len_chars());
        let count = text.chars().count();

        if coalesce && count == 1 && self.can_coalesce() {
            if let Some(EditOp::Insert { at: last_at, text: last_text }) = self.undo.last_mut() {
                if *last_at + last_text.chars().count() == at {
                    last_text.push_str(text);
                    self.rope.insert(at, text);
                    self.last_insert_end = Some(at + count);
                    self.last_delete_at = None;
                    self.note_edit();
                    return at + count;
                }
            }
        }

        self.rope.insert(at, text);
        self.undo.push(EditOp::Insert { at, text: text.to_string() });
        self.redo.clear();
        self.last_insert_end = Some(at + count);
        self.last_delete_at = None;
        self.note_edit();
        at + count
    }

    /// Remove `len` characters starting at `at`, keeping the removed text for undo.
    pub fn remove(&mut self, at: usize, len: usize, coalesce: bool) -> usize {
        if len == 0 || at >= self.len_chars() { return at; }
        let at = at.min(self.len_chars());
        let len = len.min(self.len_chars() - at);
        let removed = self.rope.slice(at..at + len).to_string();

        if coalesce && self.can_coalesce() && self.last_delete_at == Some(at + len) {
            if let Some(EditOp::Delete { at: d_at, len: d_len, text }) = self.undo.last_mut() {
                if *d_at == at + len {
                    *d_at = at;
                    *d_len += len;
                    let mut combined = removed.clone();
                    combined.push_str(text);
                    *text = combined;
                    self.rope.remove(at..at + len);
                    self.last_delete_at = Some(at);
                    self.last_insert_end = None;
                    self.note_edit();
                    return at;
                }
            }
        }

        self.rope.remove(at..at + len);
        self.undo.push(EditOp::Delete { at, len, text: removed });
        self.redo.clear();
        self.last_delete_at = Some(at);
        self.last_insert_end = None;
        self.note_edit();
        at
    }

    pub fn undo(&mut self) -> Option<usize> {
        let op = self.undo.pop()?;
        let cursor = match &op {
            EditOp::Insert { at, text } => {
                let len = text.chars().count();
                self.rope.remove(*at..*at + len);
                *at
            }
            EditOp::Delete { at, len, text } => {
                self.rope.insert(*at, text);
                at + len
            }
        };
        self.redo.push(op);
        self.revision = self.revision.wrapping_add(1);
        self.last_edit = None;
        self.last_insert_end = None;
        self.last_delete_at = None;
        Some(cursor)
    }

    pub fn redo(&mut self) -> Option<usize> {
        let op = self.redo.pop()?;
        let cursor = match &op {
            EditOp::Insert { at, text } => {
                self.rope.insert(*at, text);
                at + text.chars().count()
            }
            EditOp::Delete { at, len, .. } => {
                self.rope.remove(*at..*at + len);
                *at
            }
        };
        self.undo.push(op);
        self.revision = self.revision.wrapping_add(1);
        self.last_edit = None;
        self.last_insert_end = None;
        self.last_delete_at = None;
        Some(cursor)
    }

    pub fn can_undo(&self) -> bool { !self.undo.is_empty() }
    pub fn can_redo(&self) -> bool { !self.redo.is_empty() }

    // ---- Search ----------------------------------------------------------

    fn matches_query(&self, haystack: &str, query: &str, case: bool, whole_word: bool) -> Vec<(usize, usize)> {
        if query.is_empty() { return vec![]; }
        let (h, q) = if case {
            (haystack.to_string(), query.to_string())
        } else {
            (haystack.to_lowercase(), query.to_lowercase())
        };
        let mut out = Vec::new();
        let mut offset = 0usize;
        while let Some(pos) = h[offset..].find(&q) {
            let start = offset + pos;
            let end = start + q.chars().count();
            if whole_word {
                let before = h[..start].chars().next_back();
                let after = h[end..].chars().next();
                let is_word = |c: Option<char>| c.map(|c| c.is_alphanumeric() || c == '_').unwrap_or(false);
                if !is_word(before) && !is_word(after) {
                    out.push((start, end));
                }
            } else {
                out.push((start, end));
            }
            offset = start + q.chars().count().max(1);
            if offset > h.len() { break; }
        }
        out
    }

    /// Find the next match at or after `from`, wrapping to the start.
    pub fn find_next(&self, query: &str, from: usize, case: bool, whole_word: bool) -> Option<(usize, usize)> {
        if query.is_empty() { return None; }
        for line in 0..self.len_lines() {
            let text = self.line_text(line);
            let base = self.line_to_char(line);
            for (s, e) in self.matches_query(&text, query, case, whole_word) {
                let start = base + s;
                if start >= from {
                    return Some((start, base + e));
                }
            }
        }
        // Wrap around.
        for line in 0..self.len_lines() {
            let text = self.line_text(line);
            let base = self.line_to_char(line);
            if let Some((s, e)) = self.matches_query(&text, query, case, whole_word).first() {
                return Some((base + s, base + e));
            }
        }
        None
    }

    pub fn find_all(&self, query: &str, case: bool, whole_word: bool) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for line in 0..self.len_lines() {
            let text = self.line_text(line);
            let base = self.line_to_char(line);
            for (s, e) in self.matches_query(&text, query, case, whole_word) {
                out.push((base + s, base + e));
            }
        }
        out
    }

    pub fn replace_range(&mut self, start: usize, end: usize, text: &str) {
        self.remove(start, end - start, false);
        self.insert(start, text, false);
    }

    // ---- Persistence -----------------------------------------------------

    pub fn save(&mut self) -> io::Result<()> {
        let path = self.path.clone().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "buffer has no file path")
        })?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, self.text().as_bytes())?;
        self.dirty = false;
        Ok(())
    }

    pub fn save_as(&mut self, path: PathBuf) -> io::Result<()> {
        self.set_path(path);
        self.save()
    }

    pub fn mark_clean(&mut self) { self.dirty = false; }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_undo_redo() {
        let mut b = Buffer::from_text("hello", Language::PlainText);
        b.insert(5, " world", false);
        assert_eq!(b.text(), "hello world");
        assert_eq!(b.undo(), Some(5));
        assert_eq!(b.text(), "hello");
        assert_eq!(b.redo(), Some(11));
        assert_eq!(b.text(), "hello world");
    }

    #[test]
    fn unicode_positions_are_char_based() {
        let mut b = Buffer::from_text("héllo 世界", Language::PlainText);
        assert_eq!(b.len_chars(), 8);
        b.insert(8, "!", false);
        assert_eq!(b.text(), "héllo 世界!");
        assert_eq!(b.line_content_len(0), 9);
    }

    #[test]
    fn backspace_across_lines() {
        let mut b = Buffer::from_text("ab\ncd", Language::PlainText);
        b.remove(2, 1, false); // remove newline
        assert_eq!(b.text(), "abcd");
        b.undo();
        assert_eq!(b.text(), "ab\ncd");
    }

    #[test]
    fn search_and_replace() {
        let b = Buffer::from_text("foo bar foo", Language::PlainText);
        assert_eq!(b.find_all("foo", false, false), vec![(0, 3), (8, 11)]);
        let mut b = b;
        b.replace_range(0, 3, "baz");
        assert_eq!(b.text(), "baz bar foo");
    }
}
