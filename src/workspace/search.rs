use crate::services::Config;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub path: PathBuf,
    pub line: usize,
    pub col: usize,
    pub text: String,
    pub matches: Vec<(usize, usize)>,
}

fn find_all(haystack: &str, query: &str, case: bool, whole_word: bool) -> Vec<(usize, usize)> {
    if query.is_empty() { return Vec::new(); }
    let (h, q) = if case {
        (haystack.to_string(), query.to_string())
    } else {
        (haystack.to_lowercase(), query.to_lowercase())
    };
    let mut out = Vec::new();
    let mut offset = 0usize;
    while let Some(pos) = h[offset..].find(&q) {
        let start = offset + pos;
        let end = start + q.chars().count();
        if whole_word {
            let before = h[..start].chars().next_back();
            let after = h[end..].chars().next();
            let is_word = |c: Option<char>| c.map(|c| c.is_alphanumeric() || c == '_').unwrap_or(false);
            if !is_word(before) && !is_word(after) {
                out.push((start, end));
            }
        } else {
            out.push((start, end));
        }
        offset = start + q.chars().count().max(1);
        if offset > h.len() { break; }
    }
    out
}

/// Search file contents under `root`, honoring .gitignore and configured
/// exclusion patterns. Returns at most `limit` results.
pub fn search(
    root: &Path,
    query: &str,
    config: &Config,
    case: bool,
    whole_word: bool,
    limit: usize,
) -> Vec<SearchResult> {
    if query.is_empty() { return Vec::new(); }
    let mut results = Vec::new();
    let walker = ignore::WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .parents(true)
        .max_depth(Some(32))
        .build();

    for entry in walker.filter_map(Result::ok) {
        if results.len() >= limit { break; }
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) { continue; }
        let path = entry.path();
        let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        if config.exclude.iter().any(|p| {
            if let Some(s) = p.strip_prefix('*') { name.ends_with(s) } else { name == *p }
        }) { continue; }
        let bytes = match std::fs::read(path) {
            Ok(b) if b.len() < 8 * 1024 * 1024 => b,
            _ => continue,
        };
        if bytes.contains(&0) { continue; } // binary
        let text = String::from_utf8_lossy(&bytes);
        for (line_idx, line) in text.lines().enumerate() {
            let matches = find_all(line, query, case, whole_word);
            if !matches.is_empty() {
                results.push(SearchResult {
                    path: path.to_path_buf(),
                    line: line_idx + 1,
                    col: matches[0].0 + 1,
                    text: line.to_string(),
                    matches,
                });
                if results.len() >= limit { break; }
            }
        }
    }
    results
}
