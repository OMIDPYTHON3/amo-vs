use super::ShellConfig;
use std::path::Path;

fn find_in_path(name: &str) -> Option<String> {
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }
    None
}

pub fn default_shell() -> ShellConfig {
    if let Some(pwsh) = find_in_path("pwsh.exe") {
        return ShellConfig { name: "PowerShell 7".into(), program: pwsh, args: vec!["-NoLogo".into()] };
    }
    if let Some(ps) = find_in_path("powershell.exe") {
        return ShellConfig { name: "PowerShell".into(), program: ps, args: vec!["-NoLogo".into()] };
    }
    if let Some(cmd) = find_in_path("cmd.exe") {
        return ShellConfig { name: "Command Prompt".into(), program: cmd, args: vec![] };
    }
    ShellConfig { name: "cmd".into(), program: "cmd.exe".into(), args: vec![] }
}

pub fn command_shell() -> ShellConfig {
    if let Some(ps) = find_in_path("powershell.exe") {
        return ShellConfig { name: "PowerShell".into(), program: ps, args: vec!["-NoProfile".into(), "-Command".into()] };
    }
    ShellConfig { name: "cmd".into(), program: "cmd.exe".into(), args: vec!["/C".into()] }
}

/// Windows path helper: converts `/` to `\` for display when appropriate.
pub fn display_native(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\")
}
