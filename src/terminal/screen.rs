use ratatui::style::{Color, Modifier, Style};

#[derive(Clone, Debug)]
struct Cell {
    ch: char,
    style: Style,
}

impl Default for Cell {
    fn default() -> Self {
        Self { ch: ' ', style: Style::default() }
    }
}

#[derive(PartialEq)]
enum ParseState {
    Ground,
    Escape,
    Csi,
    Osc,
    StringEscape,
    Utf8,
}

/// A compact ANSI terminal emulator. It maintains a character grid with
/// styles so shell programs (including full-screen ones) render with colors.
pub struct AnsiScreen {
    pub cols: usize,
    pub rows: usize,
    cells: Vec<Vec<Cell>>,
    cursor_row: usize,
    cursor_col: usize,
    saved_cursor: (usize, usize),
    style: Style,
    fg: Option<Color>,
    bg: Option<Color>,
    state: ParseState,
    params: String,
    string_buf: Vec<u8>,
    utf8_buf: Vec<u8>,
    scrollback: Vec<Vec<Cell>>,
    title: String,
}

impl AnsiScreen {
    pub fn new(cols: usize, rows: usize) -> Self {
        let cols = cols.max(1);
        let rows = rows.max(1);
        Self {
            cols,
            rows,
            cells: vec![vec![Cell::default(); cols]; rows],
            cursor_row: 0,
            cursor_col: 0,
            saved_cursor: (0, 0),
            style: Style::default(),
            fg: None,
            bg: None,
            state: ParseState::Ground,
            params: String::new(),
            string_buf: Vec::new(),
            utf8_buf: Vec::new(),
            scrollback: Vec::new(),
            title: String::new(),
        }
    }

    pub fn title(&self) -> &str { &self.title }

    pub fn cursor(&self) -> (usize, usize) { (self.cursor_row, self.cursor_col) }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        let cols = cols.max(1);
        let rows = rows.max(1);
        if cols == self.cols && rows == self.rows { return; }
        // Rewrap existing rows into the new width, preserving content.
        let mut flat: Vec<Cell> = Vec::new();
        for row in &self.cells { flat.extend(row.iter().cloned()); }
        let mut cells = vec![vec![Cell::default(); cols]; rows];
        for (i, cell) in flat.into_iter().take(cols * rows).enumerate() {
            cells[i / cols][i % cols] = cell;
        }
        self.cells = cells;
        self.cols = cols;
        self.rows = rows;
        self.cursor_row = self.cursor_row.min(rows - 1);
        self.cursor_col = self.cursor_col.min(cols - 1);
    }

    fn scroll_up(&mut self) {
        let top = self.cells.remove(0);
        self.scrollback.push(top);
        if self.scrollback.len() > 2000 { self.scrollback.remove(0); }
        self.cells.push(vec![Cell::default(); self.cols]);
    }

    fn put_char(&mut self, ch: char) {
        if self.cursor_col >= self.cols {
            self.cursor_col = 0;
            self.cursor_row += 1;
            if self.cursor_row >= self.rows { self.scroll_up(); self.cursor_row = self.rows - 1; }
        }
        self.cells[self.cursor_row][self.cursor_col] = Cell { ch, style: self.style };
        self.cursor_col += 1;
    }

    fn line_feed(&mut self) {
        self.cursor_row += 1;
        if self.cursor_row >= self.rows { self.scroll_up(); self.cursor_row = self.rows - 1; }
    }

    fn control(&mut self, b: u8) {
        match b {
            0x08 => self.cursor_col = self.cursor_col.saturating_sub(1),
            0x09 => {
                let next = ((self.cursor_col / 8) + 1) * 8;
                self.cursor_col = next.min(self.cols - 1);
            }
            0x0a | 0x0b | 0x0c => self.line_feed(),
            0x0d => self.cursor_col = 0,
            0x07 => {}
            _ => {}
        }
    }

    /// Feed a chunk of bytes emitted by the shell.
    pub fn feed(&mut self, bytes: &[u8]) {
        for &b in bytes {
            match self.state {
                ParseState::Utf8 => {
                    self.utf8_buf.push(b);
                    match std::str::from_utf8(&self.utf8_buf) {
                        Ok(s) => {
                            if let Some(ch) = s.chars().next() { self.put_char(ch); }
                            self.utf8_buf.clear();
                            self.state = ParseState::Ground;
                        }
                        Err(e) if e.error_len().is_some() => {
                            self.utf8_buf.clear();
                            self.state = ParseState::Ground;
                        }
                        Err(_) => {}
                    }
                }
                ParseState::Escape => {
                    match b {
                        b'[' => { self.params.clear(); self.state = ParseState::Csi; }
                        b']' => { self.string_buf.clear(); self.state = ParseState::Osc; }
                        b'P' | b'X' | b'^' | b'_' => { self.string_buf.clear(); self.state = ParseState::StringEscape; }
                        b'\\' => self.state = ParseState::Ground,
                        0x00..=0x1f => self.control(b),
                        _ => {
                            self.state = ParseState::Ground;
                            if b >= 0x80 { self.start_utf8(b); } else { self.put_char(b as char); }
                        }
                    }
                }
                ParseState::Csi => {
                    if (0x40..=0x7e).contains(&b) {
                        self.csi(b);
                        self.params.clear();
                        self.state = ParseState::Ground;
                    } else {
                        self.params.push(b as char);
                    }
                }
                ParseState::Osc => {
                    if b == 0x07 {
                        self.handle_osc();
                        self.state = ParseState::Ground;
                    } else if b == 0x1b {
                        self.state = ParseState::StringEscape;
                    } else {
                        self.string_buf.push(b);
                    }
                }
                ParseState::StringEscape => {
                    if b == b'\\' {
                        if !self.string_buf.is_empty() { self.handle_osc(); }
                        self.state = ParseState::Ground;
                    } else if b == 0x07 {
                        self.state = ParseState::Ground;
                    } else {
                        self.string_buf.push(b);
                    }
                }
                ParseState::Ground => {
                    match b {
                        0x1b => self.state = ParseState::Escape,
                        0x00..=0x1f => self.control(b),
                        0x20..=0x7e => self.put_char(b as char),
                        _ => self.start_utf8(b),
                    }
                }
            }
        }
    }

    fn start_utf8(&mut self, b: u8) {
        self.utf8_buf.clear();
        self.utf8_buf.push(b);
        let need = if b < 0xc0 { 1 } else if b < 0xe0 { 2 } else if b < 0xf0 { 3 } else { 4 };
        if need == 1 {
            self.state = ParseState::Ground;
        } else {
            self.state = ParseState::Utf8;
        }
    }

    fn handle_osc(&mut self) {
        let s = String::from_utf8_lossy(&self.string_buf).to_string();
        let mut parts = s.splitn(2, ';');
        match parts.next().unwrap_or("") {
            "0" | "2" => self.title = parts.next().unwrap_or("").to_string(),
            _ => {}
        }
        self.string_buf.clear();
    }

    fn nums(&self) -> Vec<usize> {
        self.params
            .split(';')
            .map(|p| p.parse::<usize>().unwrap_or(0))
            .collect()
    }

    fn csi(&mut self, final_byte: u8) {
        let n = self.nums();
        let arg = |i: usize, default: usize| -> usize {
            n.get(i).copied().filter(|v| *v > 0).unwrap_or(default)
        };
        match final_byte {
            b'm' => self.sgr(&n),
            b'A' => self.cursor_row = self.cursor_row.saturating_sub(arg(0, 1)),
            b'B' => self.cursor_row = (self.cursor_row + arg(0, 1)).min(self.rows - 1),
            b'C' => self.cursor_col = (self.cursor_col + arg(0, 1)).min(self.cols - 1),
            b'D' => self.cursor_col = self.cursor_col.saturating_sub(arg(0, 1)),
            b'E' => {
                self.cursor_row = (self.cursor_row + arg(0, 1)).min(self.rows - 1);
                self.cursor_col = 0;
            }
            b'F' => {
                self.cursor_row = self.cursor_row.saturating_sub(arg(0, 1));
                self.cursor_col = 0;
            }
            b'G' | b'`' => self.cursor_col = arg(0, 1).saturating_sub(1).min(self.cols - 1),
            b'd' => self.cursor_row = arg(0, 1).saturating_sub(1).min(self.rows - 1),
            b'H' | b'f' => {
                self.cursor_row = arg(0, 1).saturating_sub(1).min(self.rows - 1);
                self.cursor_col = arg(1, 1).saturating_sub(1).min(self.cols - 1);
            }
            b'J' => {
                let mode = n.first().copied().unwrap_or(0);
                match mode {
                    0 => {
                        for c in self.cursor_col..self.cols { self.cells[self.cursor_row][c] = Cell::default(); }
                        for r in (self.cursor_row + 1)..self.rows {
                            self.cells[r] = vec![Cell::default(); self.cols];
                        }
                    }
                    1 => {
                        for c in 0..=self.cursor_col.min(self.cols - 1) { self.cells[self.cursor_row][c] = Cell::default(); }
                        for r in 0..self.cursor_row { self.cells[r] = vec![Cell::default(); self.cols]; }
                    }
                    2 | 3 => {
                        self.cells = vec![vec![Cell::default(); self.cols]; self.rows];
                        self.cursor_row = 0;
                        self.cursor_col = 0;
                    }
                    _ => {}
                }
            }
            b'K' => {
                let mode = n.first().copied().unwrap_or(0);
                match mode {
                    0 => for c in self.cursor_col..self.cols { self.cells[self.cursor_row][c] = Cell::default(); },
                    1 => for c in 0..=self.cursor_col.min(self.cols - 1) { self.cells[self.cursor_row][c] = Cell::default(); },
                    2 => self.cells[self.cursor_row] = vec![Cell::default(); self.cols],
                    _ => {}
                }
            }
            b's' => self.saved_cursor = (self.cursor_row, self.cursor_col),
            b'u' => {
                self.cursor_row = self.saved_cursor.0.min(self.rows - 1);
                self.cursor_col = self.saved_cursor.1.min(self.cols - 1);
            }
            b'h' | b'l' => {
                // Alternate screen (1049) clears our grid on toggle, which is
                // sufficient to keep full-screen programs readable.
                if n.iter().any(|v| *v == 1049) {
                    self.cells = vec![vec![Cell::default(); self.cols]; self.rows];
                    self.cursor_row = 0;
                    self.cursor_col = 0;
                }
            }
            _ => {}
        }
    }

    fn sgr(&mut self, params: &[usize]) {
        if params.is_empty() || params.len() == 1 && params[0] == 0 {
            self.style = Style::default();
            self.fg = None;
            self.bg = None;
            return;
        }
        let mut i = 0;
        while i < params.len() {
            let p = params[i];
            match p {
                0 => { self.style = Style::default(); self.fg = None; self.bg = None; }
                1 => self.style = self.style.add_modifier(Modifier::BOLD),
                2 => self.style = self.style.add_modifier(Modifier::DIM),
                3 => self.style = self.style.add_modifier(Modifier::ITALIC),
                4 => self.style = self.style.add_modifier(Modifier::UNDERLINED),
                5 => self.style = self.style.add_modifier(Modifier::SLOW_BLINK),
                7 => self.style = self.style.add_modifier(Modifier::REVERSED),
                8 => self.style = self.style.add_modifier(Modifier::HIDDEN),
                9 => self.style = self.style.add_modifier(Modifier::CROSSED_OUT),
                21 | 22 => self.style = self.style.remove_modifier(Modifier::BOLD).remove_modifier(Modifier::DIM),
                23 => self.style = self.style.remove_modifier(Modifier::ITALIC),
                24 => self.style = self.style.remove_modifier(Modifier::UNDERLINED),
                27 => self.style = self.style.remove_modifier(Modifier::REVERSED),
                28 => self.style = self.style.remove_modifier(Modifier::HIDDEN),
                29 => self.style = self.style.remove_modifier(Modifier::CROSSED_OUT),
                30..=37 => self.fg = Some(ansi_color(p - 30)),
                38 => {
                    if let Some((color, used)) = extended_color(params, i + 1) {
                        self.fg = Some(color);
                        i += used;
                    }
                }
                39 => self.fg = None,
                40..=47 => self.bg = Some(ansi_color(p - 40)),
                48 => {
                    if let Some((color, used)) = extended_color(params, i + 1) {
                        self.bg = Some(color);
                        i += used;
                    }
                }
                49 => self.bg = None,
                90..=97 => self.fg = Some(bright_color(p - 90)),
                100..=107 => self.bg = Some(bright_color(p - 100)),
                _ => {}
            }
            i += 1;
        }
        self.style = self.style.fg(self.fg.unwrap_or(Color::Reset)).bg(self.bg.unwrap_or(Color::Reset));
    }

    /// Return the visible grid as rows of styled characters.
    pub fn lines(&self) -> Vec<Vec<(char, Style)>> {
        self.cells
            .iter()
            .map(|row| row.iter().map(|c| (c.ch, c.style)).collect())
            .collect()
    }

    pub fn scrollback_len(&self) -> usize { self.scrollback.len() }
}

fn ansi_color(i: usize) -> Color {
    const COLORS: [Color; 8] = [
        Color::Black, Color::Red, Color::Green, Color::Yellow,
        Color::Blue, Color::Magenta, Color::Cyan, Color::Gray,
    ];
    COLORS.get(i).copied().unwrap_or(Color::Reset)
}

fn bright_color(i: usize) -> Color {
    const COLORS: [Color; 8] = [
        Color::DarkGray, Color::LightRed, Color::LightGreen, Color::LightYellow,
        Color::LightBlue, Color::LightMagenta, Color::LightCyan, Color::White,
    ];
    COLORS.get(i).copied().unwrap_or(Color::Reset)
}

fn extended_color(params: &[usize], start: usize) -> Option<(Color, usize)> {
    match params.get(start).copied()? {
        5 => {
            let idx = *params.get(start + 1)? as u8;
            Some((Color::Indexed(idx), 2))
        }
        2 => {
            let r = *params.get(start + 1)? as u8;
            let g = *params.get(start + 2)? as u8;
            let b = *params.get(start + 3)? as u8;
            Some((Color::Rgb(r, g, b), 4))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_and_newline() {
        let mut s = AnsiScreen::new(10, 2);
        s.feed(b"ab\r\ncd");
        let lines = s.lines();
        assert_eq!(lines[0][0].0, 'a');
        assert_eq!(lines[0][1].0, 'b');
        assert_eq!(lines[1][0].0, 'c');
    }

    #[test]
    fn handles_color_escape() {
        let mut s = AnsiScreen::new(10, 2);
        s.feed(b"\x1b[31mX\x1b[0m");
        assert_eq!(s.lines()[0][0].0, 'X');
    }

    #[test]
    fn wide_character_does_not_panic() {
        let mut s = AnsiScreen::new(4, 2);
        s.feed("世界".as_bytes());
        assert_eq!(s.lines()[0][0].0, '世');
    }
}
