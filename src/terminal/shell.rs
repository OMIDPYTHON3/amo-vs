use crate::platform;
use crossterm::event::{KeyCode, KeyModifiers};
use std::path::PathBuf;

/// Encode a key press into the byte sequence expected by a shell/PTY.
pub fn encode_key(key: KeyCode, mods: KeyModifiers) -> Vec<u8> {
    let ctrl = mods.contains(KeyModifiers::CONTROL);
    let alt = mods.contains(KeyModifiers::ALT);

    if ctrl {
        let ch = match key {
            KeyCode::Char(c) => Some(c.to_ascii_lowercase()),
            _ => None,
        };
        if let Some(c) = ch {
            let byte = if c.is_ascii_alphabetic() { c as u8 - b'a' + 1 } else { c as u8 };
            let mut out = vec![byte];
            if alt { out.insert(0, 0x1b); }
            return out;
        }
    }

    let base: Vec<u8> = match key {
        KeyCode::Char(c) => {
            let mut s = c.to_string().into_bytes();
            if alt { s.insert(0, 0x1b); }
            return s;
        }
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Left => b"\x1b[D".to_vec(),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        KeyCode::F(n) => {
            let code = match n {
                1..=5 => format!("\x1bOP"),
                6 => "\x1b[17~".into(),
                7 => "\x1b[18~".into(),
                8 => "\x1b[19~".into(),
                9 => "\x1b[20~".into(),
                10 => "\x1b[21~".into(),
                11 => "\x1b[23~".into(),
                12 => "\x1b[24~".into(),
                _ => String::new(),
            };
            code.into_bytes()
        }
        _ => Vec::new(),
    };
    base
}

pub fn history_path() -> PathBuf {
    platform::data_dir().join("terminal_history.txt")
}

pub fn load_history(limit: usize) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(history_path()) else { return Vec::new(); };
    let mut lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
    if lines.len() > limit {
        lines = lines.split_off(lines.len() - limit);
    }
    lines
}

pub fn append_history(command: &str, limit: usize) {
    let command = command.trim();
    if command.is_empty() { return; }
    let path = history_path();
    if let Some(parent) = path.parent() { platform::ensure_dir(parent); }
    let mut lines = load_history(limit);
    if lines.last().map(|s| s.as_str()) == Some(command) { return; }
    lines.push(command.to_string());
    while lines.len() > limit { lines.remove(0); }
    let _ = std::fs::write(path, lines.join("\n") + "\n");
}
