use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::*;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

/// A detected shell and the arguments needed to launch it interactively.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellConfig {
    pub name: String,
    pub program: String,
    pub args: Vec<String>,
}

/// Directory for user configuration (settings.toml, keybindings.toml, themes/).
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .map(|p| p.join("amo-vs"))
        .unwrap_or_else(|| PathBuf::from(".amo-vs"))
}

/// Directory for persistent application data (terminal history, workspace state).
pub fn data_dir() -> PathBuf {
    dirs::data_local_dir()
        .map(|p| p.join("amo-vs"))
        .unwrap_or_else(|| PathBuf::from(".amo-vs-data"))
}

pub fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// True when running inside Termux on Android.
pub fn is_termux() -> bool {
    #[cfg(target_os = "android")]
    {
        return true;
    }
    #[cfg(not(target_os = "android"))]
    {
        if std::env::var("ANDROID_ROOT").is_ok() {
            return true;
        }
        if let Ok(prefix) = std::env::var("PREFIX") {
            return prefix.contains("com.termux");
        }
        std::path::Path::new("/data/data/com.termux").exists()
    }
}

/// Shorten a path for display using VS Code style `~` substitution.
pub fn display_path(path: &std::path::Path) -> String {
    let home = home_dir();
    if let Ok(rel) = path.strip_prefix(&home) {
        let s = rel.to_string_lossy();
        return format!("~/{}", s);
    }
    path.to_string_lossy().to_string()
}

/// Ensure a directory exists, ignoring errors (best effort).
pub fn ensure_dir(path: &std::path::Path) {
    let _ = std::fs::create_dir_all(path);
}

fn recent_projects_path() -> PathBuf {
    data_dir().join("recent_projects.txt")
}

/// Load recently opened folders (most recent first).
pub fn load_recent_projects(limit: usize) -> Vec<PathBuf> {
    let Ok(text) = std::fs::read_to_string(recent_projects_path()) else { return Vec::new(); };
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .take(limit)
        .collect()
}

/// Move a folder to the top of the recent list.
pub fn remember_project(path: &Path, limit: usize) {
    if !path.is_dir() { return; }
    let path = path.to_path_buf();
    let mut projects = load_recent_projects(limit);
    projects.retain(|p| p != &path);
    projects.insert(0, path);
    projects.truncate(limit);
    let parent = recent_projects_path();
    if let Some(dir) = parent.parent() { ensure_dir(dir); }
    let text = projects.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("\n") + "\n";
    let _ = std::fs::write(parent, text);
}

/// Escape sequence used to restore a terminal to its original state.
/// Kept as a pure function so the shutdown path can be tested.
pub fn terminal_restore_sequence(mouse: bool) -> Vec<u8> {
    let mut out = Vec::new();
    if mouse {
        // Disable all mouse reporting modes enabled on startup.
        out.extend_from_slice(b"\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1015l");
    }
    out.extend_from_slice(b"\x1b[>4;0m"); // pop keyboard enhancement flags
    out.extend_from_slice(b"\x1b[?1049l"); // leave alternate screen
    out.extend_from_slice(b"\x1b[?25h");   // show cursor
    out
}
