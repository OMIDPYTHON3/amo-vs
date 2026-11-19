use crate::platform;
use crossterm::event::{KeyCode, KeyModifiers};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Icon glyph set used throughout the UI. `Simple` uses single-column Unicode
/// glyphs that work on every terminal, including Termux; `Nerd` uses Nerd Font
/// glyphs when the user's font provides them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IconSet {
    Simple,
    Nerd,
}

fn default_icon_set() -> IconSet { IconSet::Simple }

/// User-configurable key bindings. Specs are lowercase, `+`-separated, e.g.
/// `"ctrl+esc"`, `"ctrl+shift+p"`, `"alt+f4"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBindings {
    #[serde(default = "default_exit_binding")]
    pub exit: String,
    #[serde(default = "default_quit_binding")]
    pub quit: String,
    #[serde(default = "default_palette_binding")]
    pub command_palette: String,
}

fn default_exit_binding() -> String { "ctrl+esc".into() }
fn default_quit_binding() -> String { "ctrl+q".into() }
fn default_palette_binding() -> String { "f1".into() }

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            exit: default_exit_binding(),
            quit: default_quit_binding(),
            command_palette: default_palette_binding(),
        }
    }
}

impl KeyBindings {
    /// Test a crossterm key event against a binding specification.
    pub fn matches(&self, spec: &str, code: KeyCode, mods: KeyModifiers) -> bool {
        shortcut_matches(spec, code, mods)
    }
}

fn key_name(code: KeyCode) -> String {
    match code {
        KeyCode::Char(c) => c.to_ascii_lowercase().to_string(),
        KeyCode::Esc => "esc".into(),
        KeyCode::Enter => "enter".into(),
        KeyCode::Tab => "tab".into(),
        KeyCode::Backspace => "backspace".into(),
        KeyCode::Delete => "delete".into(),
        KeyCode::Up => "up".into(),
        KeyCode::Down => "down".into(),
        KeyCode::Left => "left".into(),
        KeyCode::Right => "right".into(),
        KeyCode::Home => "home".into(),
        KeyCode::End => "end".into(),
        KeyCode::PageUp => "pageup".into(),
        KeyCode::PageDown => "pagedown".into(),
        KeyCode::F(n) => format!("f{}", n),
        _ => String::new(),
    }
}

/// Return true when a key event matches a spec such as `ctrl+shift+p`.
pub fn shortcut_matches(spec: &str, code: KeyCode, mods: KeyModifiers) -> bool {
    let parts: Vec<&str> = spec.split('+').map(str::trim).filter(|s| !s.is_empty()).collect();
    if parts.is_empty() { return false; }
    let last = *parts.last().unwrap();
    if key_name(code) != last { return false; }
    let want_ctrl = parts.iter().any(|p| *p == "ctrl");
    let want_alt = parts.iter().any(|p| *p == "alt");
    let want_shift = parts.iter().any(|p| *p == "shift");
    let want_super = parts.iter().any(|p| *p == "super" || *p == "cmd");
    want_ctrl == mods.contains(KeyModifiers::CONTROL)
        && want_alt == mods.contains(KeyModifiers::ALT)
        && want_shift == mods.contains(KeyModifiers::SHIFT)
        && want_super == mods.contains(KeyModifiers::SUPER)
}

fn default_excludes() -> Vec<String> {
    vec![
        ".git".into(),
        "target".into(),
        "node_modules".into(),
        ".DS_Store".into(),
        "*.pyc".into(),
        "__pycache__".into(),
        ".amo-vs".into(),
    ]
}

/// Application settings, persisted as TOML in the user config directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_tab_size")]
    pub tab_size: usize,
    #[serde(default = "default_insert_spaces")]
    pub insert_spaces: bool,
    #[serde(default)]
    pub autosave: bool,
    #[serde(default = "default_autosave_delay")]
    pub autosave_delay_ms: u64,
    #[serde(default = "default_true")]
    pub show_line_numbers: bool,
    #[serde(default)]
    pub show_whitespace: bool,
    #[serde(default = "default_true")]
    pub highlight_current_line: bool,
    #[serde(default = "default_true")]
    pub bracket_matching: bool,
    #[serde(default = "default_true")]
    pub auto_indent: bool,
    #[serde(default)]
    pub word_wrap: bool,
    #[serde(default = "default_true")]
    pub minimap: bool,
    #[serde(default = "default_true")]
    pub render_icons: bool,
    #[serde(default = "default_true")]
    pub git_enabled: bool,
    #[serde(default = "default_true")]
    pub status_bar_visible: bool,
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: u16,
    #[serde(default = "default_panel_height")]
    pub panel_height: u16,
    #[serde(default = "default_history_size")]
    pub terminal_history_size: usize,
    #[serde(default = "default_excludes")]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub shell: Option<String>,
    #[serde(default)]
    pub keybindings: KeyBindings,
    #[serde(default = "default_icon_set")]
    pub icon_set: IconSet,
    #[serde(default = "default_true")]
    pub autocomplete: bool,
    #[serde(default = "default_completion_items")]
    pub autocomplete_max_items: usize,
    #[serde(default = "default_true")]
    pub autocomplete_workspace: bool,
}

fn default_completion_items() -> usize { 12 }

fn default_theme() -> String { "Dark+ (default dark)".into() }
fn default_tab_size() -> usize { 4 }
fn default_insert_spaces() -> bool { true }
fn default_autosave_delay() -> u64 { 1000 }
fn default_true() -> bool { true }
fn default_sidebar_width() -> u16 { 22 }
fn default_panel_height() -> u16 { 14 }
fn default_history_size() -> usize { 1000 }

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            tab_size: default_tab_size(),
            insert_spaces: default_insert_spaces(),
            autosave: false,
            autosave_delay_ms: default_autosave_delay(),
            show_line_numbers: true,
            show_whitespace: false,
            highlight_current_line: true,
            bracket_matching: true,
            auto_indent: true,
            word_wrap: false,
            minimap: true,
            render_icons: true,
            git_enabled: true,
            status_bar_visible: true,
            sidebar_width: default_sidebar_width(),
            panel_height: default_panel_height(),
            terminal_history_size: default_history_size(),
            exclude: default_excludes(),
            shell: None,
            keybindings: KeyBindings::default(),
            icon_set: default_icon_set(),
            autocomplete: true,
            autocomplete_max_items: default_completion_items(),
            autocomplete_workspace: true,
        }
    }
}

fn merge(dst: &mut toml::Value, src: toml::Value) {
    match (dst, src) {
        (toml::Value::Table(d), toml::Value::Table(s)) => {
            for (k, v) in s {
                match d.get_mut(&k) {
                    Some(existing) => merge(existing, v),
                    None => { d.insert(k, v); }
                }
            }
        }
        (d, s) => *d = s,
    }
}

impl Config {
    pub fn path() -> PathBuf {
        platform::config_dir().join("settings.toml")
    }

    pub fn load() -> Self {
        let mut base = match toml::Value::try_from(Self::default()) {
            Ok(v) => v,
            Err(_) => return Self::default(),
        };
        if let Ok(text) = std::fs::read_to_string(Self::path()) {
            if let Ok(user) = text.parse::<toml::Value>() {
                merge(&mut base, user);
            }
        }
        base.try_into::<Self>().unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let dir = platform::config_dir();
        platform::ensure_dir(&dir);
        let text = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(Self::path(), text)
    }

    /// Write a commented default configuration the first time the editor runs.
    pub fn ensure_default_file() {
        let path = Self::path();
        if path.exists() {
            return;
        }
        let dir = platform::config_dir();
        platform::ensure_dir(&dir);
        platform::ensure_dir(&crate::themes::config_path());
        let boilerplate = r#"# AMO VS settings
# All settings are optional; omitted values use the built-in defaults.

theme = "Dark+ (default dark)"
tab_size = 4
insert_spaces = true
autosave = false
autosave_delay_ms = 1000
show_line_numbers = true
show_whitespace = false
highlight_current_line = true
bracket_matching = true
auto_indent = true
word_wrap = false
minimap = true
render_icons = true
git_enabled = true
status_bar_visible = true
sidebar_width = 22
panel_height = 14
terminal_history_size = 1000
exclude = [".git", "target", "node_modules", "__pycache__", "*.pyc"]
# shell = "/bin/zsh"
"#;
        let _ = std::fs::write(path, boilerplate);
    }
}
