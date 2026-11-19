use crate::editor::buffer::Buffer;
use crate::services::Config;

pub fn matching_bracket(ch: char) -> Option<char> {
    match ch {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        ')' => Some('('),
        ']' => Some('['),
        '}' => Some('{'),
        _ => None,
    }
}

pub fn is_open(ch: char) -> bool { matches!(ch, '(' | '[' | '{') }
pub fn is_close(ch: char) -> bool { matches!(ch, ')' | ']' | '}') }

/// Find the matching bracket for the bracket at (or immediately before) `idx`.
/// Returns a character index into the buffer.
pub fn find_match(buffer: &Buffer, idx: usize, config: &Config) -> Option<usize> {
    if !config.bracket_matching { return None; }
    // Fast path: avoid materializing the whole buffer unless the cursor is
    // actually next to a bracket.
    let at_cursor = buffer.char_at(idx);
    let before_cursor = if idx > 0 { buffer.char_at(idx - 1) } else { None };
    if !at_cursor.map(|c| is_open(c) || is_close(c)).unwrap_or(false)
        && !before_cursor.map(|c| is_open(c) || is_close(c)).unwrap_or(false)
    {
        return None;
    }

    let text = buffer.text();
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() { return None; }

    let mut pos = idx.min(chars.len().saturating_sub(1));
    let candidate = if is_open(chars[pos]) || is_close(chars[pos]) {
        chars[pos]
    } else if pos > 0 && (is_open(chars[pos - 1]) || is_close(chars[pos - 1])) {
        pos -= 1;
        chars[pos]
    } else {
        return None;
    };

    let open = if is_open(candidate) { candidate } else { matching_bracket(candidate)? };
    let close = matching_bracket(open)?;
    let step: isize = if is_open(candidate) { 1 } else { -1 };
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    loop {
        pos = (pos as isize + step) as usize;
        if pos >= chars.len() { return None; }
        let c = chars[pos];
        if let Some(q) = quote {
            if c == '\\' {
                pos = (pos as isize + step) as usize;
                continue;
            }
            if c == q { quote = None; }
            continue;
        }
        if c == '"' || c == '\'' || c == '`' { quote = Some(c); continue; }
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 { return Some(pos); }
        }
    }
}

/// Leading whitespace of a line.
fn leading_ws(line: &str) -> String {
    line.chars().take_while(|c| *c == ' ' || *c == '\t').collect()
}

fn indent_unit(config: &Config) -> String {
    if config.insert_spaces { " ".repeat(config.tab_size) } else { "\t".into() }
}

/// Whitespace to insert after pressing Enter on `line`.
pub fn newline_indent(line: &str, config: &Config) -> String {
    if !config.auto_indent { return String::new(); }
    let mut indent = leading_ws(line);
    let trimmed = line.trim_end();
    let opens = trimmed.ends_with('{') || trimmed.ends_with('(') || trimmed.ends_with('[') || trimmed.ends_with(':');
    if opens {
        indent.push_str(&indent_unit(config));
    }
    indent
}

/// True when the current (whitespace-only) line should reduce indentation,
/// e.g. after typing a closing brace.
pub fn should_outdent(line: &str, config: &Config) -> bool {
    config.auto_indent && line.trim().starts_with(['}', ')', ']'])
}

/// Auto-closing pair for a typed character.
pub fn auto_close(ch: char) -> Option<char> {
    match ch {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        '"' => Some('"'),
        '\'' => Some('\''),
        '`' => Some('`'),
        _ => None,
    }
}

/// Bracket pairs currently open around `idx`, for indent guides / context.
pub fn surrounding_openers(buffer: &Buffer, idx: usize) -> Vec<char> {
    let text = buffer.text();
    let chars: Vec<char> = text.chars().collect();
    let mut stack = Vec::new();
    let mut quote: Option<char> = None;
    for &c in chars.iter().take(idx.min(chars.len())) {
        if let Some(q) = quote {
            if c == q { quote = None; }
            continue;
        }
        if c == '"' || c == '\'' || c == '`' { quote = Some(c); continue; }
        if is_open(c) { stack.push(c); }
        else if is_close(c) {
            if matching_bracket(c) == stack.last().copied() { stack.pop(); }
        }
    }
    stack
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brace_matching() {
        let b = Buffer::from_text("fn main() { if x { y(); } }", crate::editor::Language::Rust);
        let idx = b.text().chars().position(|c| c == '{').unwrap();
        let m = find_match(&b, idx, &Config::default()).unwrap();
        assert_eq!(b.text().chars().nth(m), Some('}'));
    }

    #[test]
    fn newline_indentation() {
        let cfg = Config::default();
        assert_eq!(newline_indent("    if x {", &cfg), "        ");
        assert_eq!(newline_indent("    let y = 1;", &cfg), "    ");
    }
}
