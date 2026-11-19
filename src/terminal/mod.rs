pub mod pty;
pub mod screen;
pub mod shell;

pub use pty::TerminalSession;
pub use screen::AnsiScreen;

use crate::platform::ShellConfig;
use std::path::PathBuf;

/// Manages multiple terminal sessions and the currently focused one.
pub struct TerminalManager {
    pub sessions: Vec<TerminalSession>,
    pub active: usize,
    pub height: usize,
    pub width: usize,
    next_id: usize,
    shell: Option<ShellConfig>,
    history_limit: usize,
}

impl TerminalManager {
    pub fn new(cols: usize, rows: usize, shell: Option<ShellConfig>, history_limit: usize) -> Self {
        Self {
            sessions: Vec::new(),
            active: 0,
            height: rows,
            width: cols,
            next_id: 1,
            shell,
            history_limit,
        }
    }

    pub fn ensure_session(&mut self, cwd: &PathBuf) -> std::io::Result<usize> {
        if self.sessions.is_empty() {
            self.spawn(cwd)?;
        }
        Ok(self.active)
    }

    pub fn spawn(&mut self, cwd: &PathBuf) -> std::io::Result<usize> {
        let id = self.next_id;
        self.next_id += 1;
        let session = TerminalSession::new(
            id,
            cwd.clone(),
            self.shell.clone(),
            self.width,
            self.height,
            self.history_limit,
        )?;
        self.sessions.push(session);
        self.active = self.sessions.len() - 1;
        Ok(id)
    }

    pub fn active(&self) -> Option<&TerminalSession> { self.sessions.get(self.active) }

    pub fn active_mut(&mut self) -> Option<&mut TerminalSession> { self.sessions.get_mut(self.active) }

    /// Gracefully close every session. Returns the PIDs that were terminated
    /// so callers/tests can verify no orphaned shells remain.
    pub fn close_all(&mut self) -> Vec<u32> {
        let mut pids = Vec::new();
        for session in &mut self.sessions {
            if let Some(pid) = session.pid() { pids.push(pid); }
            session.close();
        }
        self.sessions.clear();
        self.active = 0;
        pids
    }

    pub fn close_active(&mut self) {
        if let Some(session) = self.sessions.get_mut(self.active) { session.close(); }
        if self.sessions.is_empty() { return; }
        self.sessions.remove(self.active);
        if self.active >= self.sessions.len() {
            self.active = self.sessions.len().saturating_sub(1);
        }
    }

    pub fn next(&mut self) {
        if !self.sessions.is_empty() {
            self.active = (self.active + 1) % self.sessions.len();
        }
    }

    pub fn previous(&mut self) {
        if !self.sessions.is_empty() {
            self.active = (self.active + self.sessions.len() - 1) % self.sessions.len();
        }
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        if cols == self.width && rows == self.height { return; }
        self.width = cols;
        self.height = rows;
        for session in &mut self.sessions {
            session.resize(cols, rows);
        }
    }

    /// Pump every session and retire the ones whose shell has exited.
    /// Returns `true` when the UI needs a redraw (including session removal).
    pub fn pump_all(&mut self) -> bool {
        let mut changed = false;
        let mut finished = Vec::new();
        for (index, session) in self.sessions.iter_mut().enumerate() {
            changed |= session.pump();
            if !session.is_alive() {
                finished.push(index);
            }
        }
        // Remove from the end so earlier indices stay valid.
        for index in finished.into_iter().rev() {
            let mut session = self.sessions.remove(index);
            session.finalize_after_exit();
            changed = true;
        }
        if self.sessions.is_empty() {
            self.active = 0;
        } else {
            self.active = self.active.min(self.sessions.len() - 1);
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::{Duration, Instant};

    #[test]
    fn manager_removes_session_when_shell_exits() {
        let mut manager = TerminalManager::new(80, 24, None, 10);
        manager.spawn(&std::env::current_dir().unwrap()).unwrap();
        assert_eq!(manager.sessions.len(), 1);
        manager.active_mut().unwrap().write_bytes(b"exit\r");

        let deadline = Instant::now() + Duration::from_secs(3);
        let mut removed = false;
        while Instant::now() < deadline {
            if manager.pump_all() && manager.sessions.is_empty() {
                removed = true;
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(removed, "manager did not retire the terminated session");
        assert_eq!(manager.active, 0);
        assert!(manager.active().is_none());
    }
}
