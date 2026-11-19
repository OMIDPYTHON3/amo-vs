use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// Polls file metadata to detect external modifications and directory changes.
/// Polling (rather than OS watchers) keeps the dependency footprint small and
/// works identically on every supported platform, including Termux.
pub struct Watcher {
    last_scan: Instant,
    files: Vec<(PathBuf, Option<SystemTime>)>,
    dirs: Vec<(PathBuf, Option<SystemTime>)>,
}

impl Default for Watcher {
    fn default() -> Self {
        Self { last_scan: Instant::now() - Duration::from_secs(10), files: Vec::new(), dirs: Vec::new() }
    }
}

impl Watcher {
    pub fn new() -> Self { Self::default() }

    pub fn scan(&mut self, root: &Path, files: &[PathBuf]) -> bool {
        self.files = files
            .iter()
            .map(|p| {
                let mtime = std::fs::metadata(p).and_then(|m| m.modified()).ok();
                (p.clone(), mtime)
            })
            .collect();
        self.dirs.clear();
        if let Ok(read) = std::fs::read_dir(root) {
            for entry in read.filter_map(Result::ok) {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(mtime) = meta.modified() {
                        self.dirs.push((entry.path(), Some(mtime)));
                    }
                }
            }
        }
        self.last_scan = Instant::now();
        true
    }

    /// Returns true when `interval` has elapsed and something changed.
    pub fn tick(&mut self, root: &Path, files: &[PathBuf], interval: Duration) -> bool {
        if self.last_scan.elapsed() < interval { return false; }
        let mut changed = false;
        for (path, old) in &self.files {
            let now = std::fs::metadata(path).and_then(|m| m.modified()).ok();
            if now != *old { changed = true; break; }
        }
        if !changed {
            let mut current: Vec<_> = std::fs::read_dir(root)
                .map(|read| read.filter_map(Result::ok).map(|e| e.path()).collect())
                .unwrap_or_default();
            current.sort();
            let previous: Vec<_> = self.dirs.iter().map(|(p, _)| p.clone()).collect();
            changed = current != previous;
        }
        self.scan(root, files);
        changed
    }
}
