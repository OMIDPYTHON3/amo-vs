pub mod bracket;
pub mod buffer;
pub mod completion;
pub mod highlighting;
pub mod language;
pub mod view;

pub use buffer::Buffer;
pub use completion::{CompletionEngine, CompletionKind, CompletionState};
pub use highlighting::{spans_for_line, CompletionContext, DiagnosticSeverity, Highlighter, HighlightRegistry, Span, SyntaxDiagnostic};
pub use language::Language;
pub use view::EditorView;

use crate::services::Config;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// An open document: buffer, cursor/view state and syntax cache.
pub struct Editor {
    pub id: usize,
    pub buffer: Buffer,
    pub view: EditorView,
    pub highlighter: Highlighter,
    pub mtime: Option<SystemTime>,
    pub external_change: bool,
    /// True for the built-in Welcome page tab (rendered as a dedicated view).
    pub is_welcome: bool,
}

impl Editor {
    pub fn open(id: usize, path: &Path) -> io::Result<Self> {
        let buffer = Buffer::from_file(path)?;
        let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        let cursor = 0;
        Ok(Self {
            id,
            buffer,
            view: EditorView::new(cursor),
            highlighter: Highlighter::new(),
            mtime,
            external_change: false,
            is_welcome: false,
        })
    }

    pub fn scratch(id: usize, text: &str, language: Language) -> Self {
        Self {
            id,
            buffer: Buffer::from_text(text, language),
            view: EditorView::new(0),
            highlighter: Highlighter::new(),
            mtime: None,
            external_change: false,
            is_welcome: false,
        }
    }

    pub fn path(&self) -> Option<&Path> { self.buffer.path() }

    pub fn name(&self) -> String { self.buffer.name() }

    pub fn dirty(&self) -> bool { self.buffer.dirty }

    pub fn save(&mut self) -> io::Result<()> {
        self.buffer.save()?;
        self.mtime = self.path().and_then(|p| std::fs::metadata(p).ok()).and_then(|m| m.modified().ok());
        self.external_change = false;
        Ok(())
    }

    pub fn save_as(&mut self, path: PathBuf) -> io::Result<()> {
        self.buffer.save_as(path)?;
        self.mtime = self.path().and_then(|p| std::fs::metadata(p).ok()).and_then(|m| m.modified().ok());
        self.external_change = false;
        Ok(())
    }

    pub fn check_external_change(&mut self) {
        let Some(path) = self.path() else { return; };
        let Ok(meta) = std::fs::metadata(path) else {
            if !self.external_change {
                self.external_change = true;
            }
            return;
        };
        let Ok(now) = meta.modified() else { return; };
        if let Some(old) = self.mtime {
            if now != old && !self.dirty() {
                self.external_change = true;
            }
        }
    }

    pub fn reload(&mut self) -> io::Result<()> {
        let Some(path) = self.path().map(|p| p.to_path_buf()) else { return Ok(()); };
        let buffer = Buffer::from_file(&path)?;
        self.mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        self.buffer = buffer;
        self.view.cursor = self.view.cursor.min(self.buffer.len_chars());
        self.view.anchor = None;
        self.highlighter.invalidate();
        self.external_change = false;
        Ok(())
    }

    // ---- Editing commands ------------------------------------------------

    /// Type or paste text at the cursor, handling auto-close pairs.
    pub fn insert_text(&mut self, text: &str, _config: &Config) {
        if let Some((start, end)) = self.view.selection() {
            self.buffer.remove(start, end - start, false);
            self.view.cursor = start;
            self.view.anchor = None;
        }

        let chars: Vec<char> = text.chars().collect();
        if chars.len() == 1 {
            let ch = chars[0];
            // Typing over an auto-inserted closing character: step over it.
            if bracket::is_close(ch) && self.buffer.char_at(self.view.cursor) == Some(ch) {
                self.view.cursor += 1;
                self.view.desired_col += 1;
                return;
            }
            if let Some(close) = bracket::auto_close(ch) {
                let next = self.buffer.char_at(self.view.cursor);
                if next.is_none() || next.map(|c| c.is_whitespace() || bracket::is_close(c)).unwrap_or(false) {
                    let pair: String = [ch, close].iter().collect();
                    let pos = self.buffer.insert(self.view.cursor, &pair, false);
                    self.view.cursor = pos - 1;
                    self.view.desired_col = self.view.column(&self.buffer);
                    return;
                }
            }
        }

        let pos = self.buffer.insert(self.view.cursor, text, chars.len() == 1);
        self.view.cursor = pos;
        self.view.desired_col = self.view.column(&self.buffer);
    }

    pub fn backspace(&mut self, _config: &Config) {
        if let Some((start, end)) = self.view.selection() {
            self.buffer.remove(start, end - start, false);
            self.view.cursor = start;
            self.view.anchor = None;
            self.view.desired_col = self.view.column(&self.buffer);
            return;
        }
        if self.view.cursor == 0 { return; }
        let idx = self.view.cursor - 1;
        // Remove an entire auto-inserted pair in one step.
        if bracket::is_open(self.buffer.char_at(idx).unwrap_or(' ')) {
            let close = bracket::matching_bracket(self.buffer.char_at(idx).unwrap());
            if close.is_some() && self.buffer.char_at(idx + 1) == close {
                self.buffer.remove(idx, 2, false);
                self.view.cursor = idx;
                self.view.desired_col = self.view.column(&self.buffer);
                return;
            }
        }
        self.buffer.remove(idx, 1, true);
        self.view.cursor = idx;
        self.view.desired_col = self.view.column(&self.buffer);
    }

    pub fn delete(&mut self) {
        if let Some((start, end)) = self.view.selection() {
            self.buffer.remove(start, end - start, false);
            self.view.cursor = start;
            self.view.anchor = None;
            return;
        }
        if self.view.cursor < self.buffer.len_chars() {
            let idx = self.view.cursor;
            if bracket::is_open(self.buffer.char_at(idx).unwrap_or(' ')) {
                let close = bracket::matching_bracket(self.buffer.char_at(idx).unwrap());
                if close.is_some() && self.buffer.char_at(idx + 1) == close {
                    self.buffer.remove(idx, 2, false);
                    return;
                }
            }
            self.buffer.remove(idx, 1, false);
        }
    }

    pub fn newline(&mut self, config: &Config) {
        if let Some((start, end)) = self.view.selection() {
            self.buffer.remove(start, end - start, false);
            self.view.cursor = start;
            self.view.anchor = None;
        }
        let idx = self.view.cursor;
        let line = self.buffer.char_to_line(idx);
        let line_text = self.buffer.line_text(line);
        let base_indent = bracket::newline_indent(&line_text, config);

        let prev_char = if idx > 0 { self.buffer.char_at(idx - 1) } else { None };
        let next_char = self.buffer.char_at(idx);
        let between_braces = prev_char.map(bracket::is_open).unwrap_or(false)
            && next_char.map(bracket::is_close).unwrap_or(false)
            && (next_char.is_none() || matching(prev_char.unwrap(), next_char.unwrap()));

        let unit = if config.insert_spaces { " ".repeat(config.tab_size) } else { "\t".into() };
        let insert = if between_braces {
            format!("\n{}{}\n{}", base_indent, unit, base_indent.trim_end_matches(&unit))
        } else {
            format!("\n{}", base_indent)
        };
        let pos = self.buffer.insert(idx, &insert, false);
        if between_braces {
            let inner = idx + 1 + base_indent.chars().count();
            self.view.cursor = inner;
        } else {
            self.view.cursor = pos;
        }
        self.view.anchor = None;
        self.view.desired_col = self.view.column(&self.buffer);
    }

    pub fn indent(&mut self, config: &Config) {
        self.indent_lines(config, true);
    }

    pub fn outdent(&mut self, config: &Config) {
        self.indent_lines(config, false);
    }

    fn indent_lines(&mut self, config: &Config, add: bool) {
        let (start_line, end_line) = match self.view.selection() {
            Some((s, e)) => (self.buffer.char_to_line(s), self.buffer.char_to_line(e)),
            None => {
                let l = self.buffer.char_to_line(self.view.cursor);
                (l, l)
            }
        };
        let unit = if config.insert_spaces { " ".repeat(config.tab_size) } else { "\t".into() };
        for line in (start_line..=end_line.min(self.buffer.len_lines().saturating_sub(1))).rev() {
            let start = self.buffer.line_to_char(line);
            let text = self.buffer.line_text(line);
            if add {
                self.buffer.insert(start, &unit, false);
            } else {
                let remove = text.chars().take_while(|c| *c == ' ' || *c == '\t').count().min(config.tab_size.max(1));
                if remove > 0 {
                    self.buffer.remove(start, remove, false);
                }
            }
        }
        let cursor_line = self.buffer.char_to_line(self.view.cursor);
        let col = self.view.column(&self.buffer);
        self.view.cursor = self.buffer.line_to_char(cursor_line) + col;
        self.view.anchor = None;
    }

    pub fn toggle_comment(&mut self, _config: &Config) {
        let prefix = self.buffer.language.comment_prefix();
        let (start_line, end_line) = match self.view.selection() {
            Some((s, e)) => (self.buffer.char_to_line(s), self.buffer.char_to_line(e)),
            None => {
                let l = self.buffer.char_to_line(self.view.cursor);
                (l, l)
            }
        };
        let all_commented = (start_line..=end_line).all(|l| {
            self.buffer.line_text(l).trim_start().starts_with(prefix.trim_end())
        });
        for line in (start_line..=end_line.min(self.buffer.len_lines().saturating_sub(1))).rev() {
            let start = self.buffer.line_to_char(line) + self.buffer.line_text(line).chars().take_while(|c| c.is_whitespace()).count();
            let text = self.buffer.line_text(line);
            let trimmed = text.trim_start();
            if all_commented && trimmed.starts_with(prefix.trim_end()) {
                let n = prefix.trim_end().chars().count()
                    + if trimmed[prefix.trim_end().len()..].starts_with(' ') { 1 } else { 0 };
                let _ = n;
                let remove = if prefix == "//" || prefix == "#" || prefix == "--" { 3 } else { prefix.len() + 1 };
                let actual = trimmed.chars().take(remove).count().min(trimmed.chars().count());
                self.buffer.remove(start, actual, false);
            } else if !all_commented {
                self.buffer.insert(start, &format!("{} ", prefix), false);
            }
        }
        self.view.desired_col = self.view.column(&self.buffer);
    }

    pub fn find_next(&mut self, query: &str, case: bool, word: bool) -> Option<(usize, usize)> {
        let from = if self.view.has_selection() {
            self.view.selection().unwrap().1
        } else {
            self.view.cursor
        };
        let found = self.buffer.find_next(query, from, case, word)?;
        self.view.cursor = found.1;
        self.view.anchor = Some(found.0);
        self.view.desired_col = found.1 - self.buffer.line_to_char(self.buffer.char_to_line(found.1));
        Some(found)
    }

    pub fn replace_all(&mut self, query: &str, replacement: &str, case: bool, word: bool) -> usize {
        let matches = self.buffer.find_all(query, case, word);
        for (start, end) in matches.iter().rev() {
            self.buffer.replace_range(*start, *end, replacement);
        }
        self.view.anchor = None;
        self.view.cursor = self.view.cursor.min(self.buffer.len_chars());
        matches.len()
    }
}

fn matching(open: char, close: char) -> bool {
    bracket::matching_bracket(open) == Some(close)
}
