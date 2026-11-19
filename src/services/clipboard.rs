/// Clipboard implementation. Keeps an internal buffer and additionally emits an
/// OSC 52 escape sequence so the host terminal's system clipboard is updated.
/// OSC 52 works on xterm-compatible terminals, iTerm2, kitty, WezTerm and
/// Windows Terminal; the internal buffer guarantees copy/paste everywhere.
#[derive(Debug, Default)]
pub struct Clipboard {
    content: String,
}

impl Clipboard {
    pub fn new() -> Self { Self::default() }

    pub fn set(&mut self, text: String) {
        self.content = text;
        write_osc52(&self.content);
    }

    pub fn get(&self) -> String { self.content.clone() }

    pub fn is_empty(&self) -> bool { self.content.is_empty() }
}

fn write_osc52(text: &str) {
    if text.is_empty() {
        return;
    }
    let payload = base64_encode(text.as_bytes());
    let sequence = format!("\x1b]52;c;{}\x07", payload);

    #[cfg(unix)]
    {
        use std::io::Write;
        if let Ok(mut tty) = std::fs::OpenOptions::new().write(true).open("/dev/tty") {
            let _ = tty.write_all(sequence.as_bytes());
            let _ = tty.flush();
            return;
        }
    }
    // Fallback: standard output (captured by the host terminal emulator).
    print!("{}", sequence);
    use std::io::Write;
    let _ = std::io::stdout().flush();
}

fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i] as u32;
        let b1 = if i + 1 < data.len() { data[i + 1] as u32 } else { 0 };
        let b2 = if i + 2 < data.len() { data[i + 2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(if i + 1 < data.len() { TABLE[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if i + 2 < data.len() { TABLE[(n & 63) as usize] as char } else { '=' });
        i += 3;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_known_values() {
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
    }

    #[test]
    fn clipboard_roundtrip() {
        let mut c = Clipboard::new();
        c.set("hello".into());
        assert_eq!(c.get(), "hello");
    }
}
