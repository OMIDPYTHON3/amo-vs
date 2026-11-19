//! End-to-end verification of integrated-terminal shutdown. Each test runs the
//! real binary inside a PTY, opens integrated shells, starts long-running
//! commands, exits through the global shortcut and verifies that no shell or
//! child process survives and that the original terminal is usable again.

use amo_vs::terminal::AnsiScreen;
use portable_pty::{native_pty_system, Child, CommandBuilder, PtySize};
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct Harness {
    child: Box<dyn Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
    rx: Receiver<Vec<u8>>,
    reader: Option<JoinHandle<()>>,
    screen: AnsiScreen,
}

impl Harness {
    fn new(cols: u16, rows: u16, workspace: &PathBuf) -> Self {
        let pty = native_pty_system();
        let pair = pty.openpty(PtySize {
            rows, cols, pixel_width: 0, pixel_height: 0,
        }).unwrap();
        let bin = env!("CARGO_BIN_EXE_amo-vs");
        // Wrapper shell proves the original terminal is restored and usable.
        let script = format!(
            "{} {}; echo AMO_RESTORED_MARKER; exec bash",
            shell_quote(bin), shell_quote(&workspace.display().to_string())
        );
        let mut cmd = CommandBuilder::new("bash");
        cmd.arg("-c");
        cmd.arg(script);
        cmd.env("TERM", "xterm-256color");
        let child = pair.slave.spawn_command(cmd).unwrap();
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().unwrap();
        let writer = pair.master.take_writer().unwrap();
        let (tx, rx) = mpsc::channel();
        let reader = thread::spawn(move || {
            let mut buf = [0u8; 16384];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => { if tx.send(buf[..n].to_vec()).is_err() { break; } }
                }
            }
        });
        let mut h = Self {
            child, writer, rx, reader: Some(reader),
            screen: AnsiScreen::new(cols as usize, rows as usize),
        };
        h.wait(1800);
        h
    }

    fn drain(&mut self) {
        while let Ok(bytes) = self.rx.try_recv() { self.screen.feed(&bytes); }
    }

    fn wait(&mut self, millis: u64) {
        thread::sleep(Duration::from_millis(millis));
        self.drain();
    }

    fn send(&mut self, bytes: &[u8]) {
        self.writer.write_all(bytes).unwrap();
        self.writer.flush().unwrap();
    }

    fn text(&mut self) -> String {
        self.drain();
        self.screen.lines().iter()
            .map(|row| row.iter().map(|(c, _)| *c).collect::<String>())
            .collect::<Vec<_>>().join("\n")
    }

    /// Open a new integrated terminal through the command palette.
    fn open_terminal(&mut self) {
        self.send(b"\x10");            // Ctrl+P -> quick open
        self.wait(200);
        self.send(b">new terminal");   // switch to commands, filter
        self.wait(350);
        self.send(b"\r");              // accept
        self.wait(1200);
    }

    /// Start a long-running command in the focused integrated terminal and
    /// return the path of the file containing its PID.
    fn start_sleep(&mut self, marker: &PathBuf) {
        let command = format!(
            "sh -c 'echo $$ > {}; sleep 600'\r",
            shell_quote(&marker.display().to_string())
        );
        self.send(command.as_bytes());
        self.wait(600);
    }

    fn exit_app(&mut self) {
        self.send(b"\x11"); // Ctrl+Q global exit
        self.wait(1200);
    }

    fn finished(&mut self) -> bool {
        // The marker may wrap across terminal rows, so match a fragment.
        self.text().contains("RESTORED")
    }

    fn wait_until_finished(&mut self, seconds: u64) -> bool {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        while Instant::now() < deadline {
            if self.finished() { return true; }
            thread::sleep(Duration::from_millis(200));
        }
        self.finished()
    }

    fn cleanup(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(handle) = self.reader.take() { let _ = handle.join(); }
    }
}

fn workspace(name: &str) -> PathBuf {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("amo-shutdown-{}-{}-{}", std::process::id(), name, id));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("README.md"), "# Shutdown test\n").unwrap();
    fs::write(dir.join("src/main.rs"), "fn main() {}\n").unwrap();
    dir
}

fn marker_path(name: &str) -> PathBuf {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("amo-shutdown-{}-{}.pid", name, id))
}

fn read_pid(path: &PathBuf) -> Option<u32> {
    for _ in 0..30 {
        if let Ok(s) = fs::read_to_string(path) {
            if let Ok(pid) = s.trim().parse::<u32>() { return Some(pid); }
        }
        thread::sleep(Duration::from_millis(50));
    }
    None
}

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    if unsafe { libc::kill(pid as i32, 0) } != 0 { return false; }
    if let Ok(stat) = fs::read_to_string(format!("/proc/{}/stat", pid)) {
        if let Some(pos) = stat.rfind(')') {
            let state = stat[pos + 2..].chars().next().unwrap_or('R');
            if state == 'Z' { return false; }
        }
    }
    true
}

#[cfg(not(unix))]
fn process_alive(_pid: u32) -> bool { false }

fn assert_no_orphan(pid: Option<u32>, label: &str) {
    let pid = pid.unwrap_or_else(|| panic!("no PID recorded for {}", label));
    let deadline = Instant::now() + Duration::from_secs(4);
    while Instant::now() < deadline && process_alive(pid) {
        thread::sleep(Duration::from_millis(100));
    }
    assert!(!process_alive(pid), "orphaned process {} from {}", pid, label);
}

fn shell_quote(s: &str) -> String {
    if !s.contains(|c: char| c.is_whitespace() || c == '\'' || c == '"') { return s.to_string(); }
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[test]
#[cfg(unix)]
fn typing_exit_removes_terminal_session() {
    let root = workspace("selfexit");
    let marker = marker_path("selfexit");
    let mut h = Harness::new(100, 30, &root);
    h.open_terminal();
    assert!(h.text().contains("TERMINAL"), "terminal panel did not open");
    // Record the integrated shell's PID, then terminate it itself.
    let command = format!("echo $$ > {}\r", shell_quote(&marker.display().to_string()));
    h.send(command.as_bytes());
    h.wait(400);
    let pid = read_pid(&marker);

    h.send(b"exit\r");
    let deadline = Instant::now() + Duration::from_secs(6);
    let mut removed = false;
    while Instant::now() < deadline {
        if h.text().contains("No terminal sessions") {
            removed = true;
            break;
        }
        h.wait(150);
    }
    assert!(removed, "terminal session was not removed after `exit`\nSCREEN:\n{}", h.text());
    assert_no_orphan(pid, "self-exited shell");
    h.cleanup();
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_file(marker);
}

#[test]
#[cfg(unix)]
fn ctrl_d_removes_terminal_session() {
    let root = workspace("ctrld");
    let mut h = Harness::new(100, 30, &root);
    h.open_terminal();
    assert!(h.text().contains("TERMINAL"), "terminal panel did not open");
    h.send(b"\x04"); // Ctrl+D -> EOF for the shell
    let deadline = Instant::now() + Duration::from_secs(6);
    let mut removed = false;
    while Instant::now() < deadline {
        if h.text().contains("No terminal sessions") {
            removed = true;
            break;
        }
        h.wait(150);
    }
    assert!(removed, "terminal session was not removed after Ctrl+D\nSCREEN:\n{}", h.text());
    h.cleanup();
    let _ = fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn idle_integrated_shell_is_closed_and_terminal_restored() {
    let root = workspace("idle");
    let mut h = Harness::new(100, 30, &root);
    h.open_terminal();
    assert!(h.text().contains("TERMINAL"), "terminal panel did not open");
    h.exit_app();
    assert!(h.wait_until_finished(8), "wrapper shell never regained control\nSCREEN:\n{}", h.text());
    h.send(b"echo PARENT_OK\r");
    h.wait(400);
    assert!(h.text().contains("PARENT_OK"), "original shell is not usable");
    h.cleanup();
    let _ = fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn running_command_is_terminated_without_orphans() {
    let root = workspace("running");
    let marker = marker_path("running");
    let mut h = Harness::new(100, 30, &root);
    h.open_terminal();
    h.start_sleep(&marker);
    let pid = read_pid(&marker);
    h.exit_app();
    assert!(h.wait_until_finished(8), "wrapper shell never regained control\nSCREEN:\n{}", h.text());
    assert_no_orphan(pid, "running command");
    h.cleanup();
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_file(marker);
}

#[test]
#[cfg(unix)]
fn multiple_sessions_are_all_terminated() {
    let root = workspace("multiple");
    let marker1 = marker_path("multi1");
    let marker2 = marker_path("multi2");
    let mut h = Harness::new(100, 30, &root);
    h.open_terminal();
    h.start_sleep(&marker1);
    let pid1 = read_pid(&marker1);
    h.open_terminal();
    h.start_sleep(&marker2);
    let pid2 = read_pid(&marker2);
    h.exit_app();
    assert!(h.wait_until_finished(8), "wrapper shell never regained control\nSCREEN:\n{}", h.text());
    assert_no_orphan(pid1, "first session");
    assert_no_orphan(pid2, "second session");
    h.cleanup();
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_file(marker1);
    let _ = fs::remove_file(marker2);
}

#[test]
#[cfg(unix)]
fn exit_with_unsaved_changes_cleans_up_terminals() {
    let root = workspace("dirty");
    let marker = marker_path("dirty");
    let mut h = Harness::new(100, 30, &root);
    // Open a file and make it dirty.
    h.send(b"\x10");
    h.wait(200);
    h.send(b"main.rs");
    h.wait(350);
    h.send(b"\r");
    h.wait(400);
    h.send(b"//dirty ");
    h.wait(200);
    // Open a terminal with a running command.
    h.open_terminal();
    h.start_sleep(&marker);
    let pid = read_pid(&marker);
    // Exit -> confirmation dialog -> "Exit Without Saving".
    h.exit_app();
    assert!(h.text().contains("Exit AMO VS"), "exit dialog not shown");
    h.send(b"2");
    h.wait(1500);
    assert!(h.wait_until_finished(8), "wrapper shell never regained control\nSCREEN:\n{}", h.text());
    assert_no_orphan(pid, "unsaved-changes session");
    h.cleanup();
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_file(marker);
}
