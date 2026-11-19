use super::screen::AnsiScreen;
use super::shell;
use crate::platform::{self, ShellConfig};
use crossterm::event::{KeyCode, KeyModifiers};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Graceful-close time budget. The shell is first asked to exit, then the
/// whole foreground process group gets SIGTERM, and only as a last resort
/// SIGKILL. This avoids orphaned shells and "extra prompt" artefacts.
const EXIT_WAIT: Duration = Duration::from_millis(600);
const TERM_WAIT: Duration = Duration::from_millis(400);
const READER_WAIT: Duration = Duration::from_millis(300);

/// One live shell session backed by a real pseudo-terminal.
pub struct TerminalSession {
    pub id: usize,
    pub name: String,
    pub cwd: PathBuf,
    pub screen: AnsiScreen,
    pub input: String,
    pub history: Vec<String>,
    pub history_pos: usize,
    pub alive: bool,
    master: Option<Box<dyn MasterPty + Send>>,
    writer: Option<Box<dyn Write + Send>>,
    child: Option<Box<dyn Child + Send + Sync>>,
    rx: Receiver<Vec<u8>>,
    reader_done: Arc<AtomicBool>,
    reader_handle: Option<JoinHandle<()>>,
}

impl TerminalSession {
    pub fn new(
        id: usize,
        cwd: PathBuf,
        shell: Option<ShellConfig>,
        cols: usize,
        rows: usize,
        history_limit: usize,
    ) -> io::Result<Self> {
        let shell = shell.unwrap_or_else(platform::default_shell);
        let pty_system = native_pty_system();
        let size = PtySize {
            rows: rows.max(1) as u16,
            cols: cols.max(1) as u16,
            pixel_width: 0,
            pixel_height: 0,
        };
        let pair = pty_system
            .openpty(size)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        let mut cmd = CommandBuilder::new(&shell.program);
        for arg in &shell.args { cmd.arg(arg); }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.cwd(&cwd);

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        // The slave side must be closed in the parent; the child owns its copy.
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        let (tx, rx) = mpsc::channel();
        let reader_done = Arc::new(AtomicBool::new(false));
        let done_flag = Arc::clone(&reader_done);
        let reader_handle = thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() { break; }
                    }
                }
            }
            done_flag.store(true, Ordering::SeqCst);
        });

        let history = shell::load_history(history_limit);
        let history_pos = history.len();

        Ok(Self {
            id,
            name: format!("{}: {}", shell.name, id),
            cwd,
            screen: AnsiScreen::new(cols, rows),
            input: String::new(),
            history,
            history_pos,
            alive: true,
            master: Some(pair.master),
            writer: Some(writer),
            child: Some(child),
            rx,
            reader_done,
            reader_handle: Some(reader_handle),
        })
    }

    /// Drain pending output and detect shell termination. Both signals matter:
    /// `try_wait` observes the child process exiting, while the reader thread
    /// reports PTY EOF once the last slave descriptor closes (as happens when
    /// the shell exits via `exit` or Ctrl+D). Either one marks the session as
    /// terminated so the manager can reap and remove it.
    pub fn pump(&mut self) -> bool {
        let mut updated = false;
        while let Ok(bytes) = self.rx.try_recv() {
            self.screen.feed(&bytes);
            updated = true;
        }
        if self.alive {
            let child_exited = match &mut self.child {
                Some(child) => matches!(child.try_wait(), Ok(Some(_)) | Err(_)),
                None => true,
            };
            let pty_eof = self.reader_done.load(Ordering::SeqCst);
            if child_exited || pty_eof {
                self.alive = false;
                updated = true; // force a redraw so the session disappears
            }
        }
        updated
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        if let Some(writer) = &mut self.writer {
            if writer.write_all(bytes).is_ok() {
                let _ = writer.flush();
            }
        }
    }

    pub fn send_key(&mut self, key: KeyCode, mods: KeyModifiers) {
        // Track the current command line for shell history. The shell itself
        // handles echo and editing; this is only used for recall.
        match key {
            KeyCode::Char(c) if !mods.contains(KeyModifiers::CONTROL) && !mods.contains(KeyModifiers::ALT) => {
                self.input.push(c);
            }
            KeyCode::Backspace => { self.input.pop(); }
            _ => {}
        }
        let bytes = shell::encode_key(key, mods);
        if !bytes.is_empty() {
            self.write_bytes(&bytes);
        }
    }

    pub fn submit(&mut self, history_limit: usize) {
        let cmd = self.input.clone();
        shell::append_history(&cmd, history_limit);
        if !cmd.trim().is_empty() {
            if self.history.last().map(|s| s.as_str()) != Some(cmd.as_str()) {
                self.history.push(cmd);
            }
        }
        self.history_pos = self.history.len();
        self.input.clear();
        self.write_bytes(b"\r");
    }

    pub fn history_prev(&mut self) {
        if self.history.is_empty() { return; }
        self.history_pos = self.history_pos.saturating_sub(1);
        self.input = self.history.get(self.history_pos).cloned().unwrap_or_default();
    }

    pub fn history_next(&mut self) {
        if self.history.is_empty() { return; }
        if self.history_pos + 1 < self.history.len() {
            self.history_pos += 1;
            self.input = self.history[self.history_pos].clone();
        } else {
            self.history_pos = self.history.len();
            self.input.clear();
        }
    }

    pub fn pid(&self) -> Option<u32> { self.child.as_ref().and_then(|c| c.process_id()) }

    pub fn is_alive(&self) -> bool { self.alive }

    /// Gracefully terminate the shell and its process group, reap the child,
    /// close both PTY ends and stop the reader thread.
    ///
    /// Order matters: ask the shell to exit, wait briefly, escalate to SIGTERM
    /// for the whole foreground group (so long-running commands die too),
    /// then use SIGKILL only if the process still refuses to exit. PTY handles
    /// are dropped afterwards so the reader's blocking read unblocks.
    pub fn close(&mut self) {
        // 1. Ask the shell to log out cleanly.
        if self.alive {
            self.write_bytes(b"exit\r");
        }

        // 2. Wait for a clean exit.
        if self.wait_for_exit(EXIT_WAIT) {
            self.finish_close();
            return;
        }

        // 3. Escalate: SIGTERM the whole process group (shell + children).
        #[cfg(unix)]
        if let Some(pid) = self.pid() {
            signal_group(pid, 15); // SIGTERM
        }
        #[cfg(not(unix))]
        {
            // Windows ConPTY: children are in a job object; killing the root
            // process terminates the whole tree.
            if let Some(child) = &mut self.child { let _ = child.kill(); }
        }
        if self.wait_for_exit(TERM_WAIT) {
            self.finish_close();
            return;
        }

        // 4. Last resort: SIGKILL the group and reap.
        #[cfg(unix)]
        if let Some(pid) = self.pid() {
            signal_group(pid, 9); // SIGKILL
        }
        #[cfg(not(unix))]
        if let Some(child) = &mut self.child { let _ = child.kill(); }
        if let Some(child) = &mut self.child { let _ = child.wait(); }
        self.alive = false;
        self.finish_close();
    }

    fn wait_for_exit(&mut self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(child) = &mut self.child {
                match child.try_wait() {
                    Ok(Some(_)) => {
                        self.alive = false;
                        return true;
                    }
                    Ok(None) => {}
                    Err(_) => {
                        self.alive = false;
                        return true;
                    }
                }
            }
            if Instant::now() >= deadline { return false; }
            thread::sleep(Duration::from_millis(10));
        }
    }

    /// Reap the shell and release every PTY resource after it has terminated
    /// on its own (no signals needed). Safe to call from the manager when
    /// `pump()` reports the session is gone.
    pub fn finalize_after_exit(&mut self) {
        self.alive = false;
        self.finish_close();
    }

    fn finish_close(&mut self) {
        // Reap the child so no zombie is left behind. `wait()` returns
        // immediately when `try_wait()` already consumed the exit status.
        if let Some(child) = &mut self.child {
            let _ = child.wait();
        }
        self.child = None;
        // Drain anything still buffered so the reader channel is empty.
        while self.rx.try_recv().is_ok() {}
        // Closing the master end unblocks the reader thread with an error.
        self.writer.take();
        self.master.take();

        let deadline = Instant::now() + READER_WAIT;
        while !self.reader_done.load(Ordering::SeqCst) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        if self.reader_done.load(Ordering::SeqCst) {
            if let Some(handle) = self.reader_handle.take() { let _ = handle.join(); }
        } else {
            // Detach a stuck reader rather than blocking application exit.
            self.reader_handle.take();
        }
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        let cols = cols.max(1);
        let rows = rows.max(1);
        let size = PtySize {
            rows: rows as u16,
            cols: cols as u16,
            pixel_width: 0,
            pixel_height: 0,
        };
        if let Some(master) = &mut self.master { let _ = master.resize(size); }
        self.screen.resize(cols, rows);
    }
}

#[cfg(unix)]
fn signal_group(pid: u32, signal: i32) {
    // portable-pty makes the shell a session/process-group leader, so a
    // negative pid signals the entire foreground job as well.
    unsafe {
        libc::kill(-(pid as i32), signal);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_exit_is_detected_and_resources_released() {
        let mut session = TerminalSession::new(2, std::env::current_dir().unwrap(), None, 80, 24, 10).unwrap();
        let pid = session.pid().unwrap();
        // Ask the shell to terminate itself (equivalent to typing `exit`).
        session.write_bytes(b"exit\r");
        let deadline = Instant::now() + Duration::from_secs(3);
        while session.is_alive() && Instant::now() < deadline {
            let _ = session.pump();
            thread::sleep(Duration::from_millis(10));
        }
        assert!(!session.is_alive(), "shell exit was not detected");
        session.finalize_after_exit();
        assert!(session.writer.is_none());
        assert!(session.master.is_none());
        assert!(session.child.is_none());
        #[cfg(unix)]
        assert_ne!(unsafe { libc::kill(pid as i32, 0) }, 0, "shell still alive after exit");
    }

    #[test]
    fn session_spawns_and_closes_gracefully() {
        let mut session = TerminalSession::new(1, std::env::current_dir().unwrap(), None, 80, 24, 10).unwrap();
        assert!(session.is_alive());
        assert!(session.pid().is_some());
        let pid = session.pid().unwrap();
        session.close();
        assert!(!session.is_alive());
        assert!(session.writer.is_none());
        assert!(session.master.is_none());
        // The shell must be gone (kill(pid, 0) fails with ESRCH).
        #[cfg(unix)]
        {
            let rc = unsafe { libc::kill(pid as i32, 0) };
            assert_ne!(rc, 0, "shell process still exists after close");
        }
    }
}
