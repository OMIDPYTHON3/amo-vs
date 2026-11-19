use super::ShellConfig;
use std::path::Path;

fn executable(name: &str) -> bool {
    if name.starts_with('/') {
        return Path::new(name).is_file();
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            if dir.join(name).is_file() {
                return true;
            }
        }
    }
    false
}

/// Detect the user's preferred shell, honoring the Termux environment.
pub fn default_shell() -> ShellConfig {
    if super::is_termux() {
        let candidates = [
            "/data/data/com.termux/files/usr/bin/bash",
            "/data/data/com.termux/files/usr/bin/zsh",
            "/data/data/com.termux/files/usr/bin/sh",
        ];
        for c in candidates {
            if Path::new(c).exists() {
                let name = Path::new(c)
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "shell".into());
                return ShellConfig { name, program: c.into(), args: vec!["-i".into()] };
            }
        }
    }

    if let Ok(shell) = std::env::var("SHELL") {
        if !shell.is_empty() && Path::new(&shell).is_file() {
            let name = Path::new(&shell)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "shell".into());
            return ShellConfig { name, program: shell, args: vec!["-i".into()] };
        }
    }

    for candidate in ["/bin/bash", "/usr/bin/bash", "/bin/zsh", "/usr/bin/zsh", "/bin/sh"] {
        if Path::new(candidate).exists() {
            let name = Path::new(candidate)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "shell".into());
            return ShellConfig { name, program: candidate.into(), args: vec!["-i".into()] };
        }
    }

    ShellConfig { name: "sh".into(), program: "sh".into(), args: vec!["-i".into()] }
}

/// Shell used for single non-interactive commands from the command palette.
pub fn command_shell() -> ShellConfig {
    if executable("bash") {
        ShellConfig { name: "bash".into(), program: "bash".into(), args: vec!["-lc".into()] }
    } else {
        ShellConfig { name: "sh".into(), program: "sh".into(), args: vec!["-c".into()] }
    }
}
