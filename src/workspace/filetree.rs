use super::GitState;
use crate::services::git::GitInfo;
use crate::editor::Language;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FileNode {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub expanded: bool,
    pub children: Vec<FileNode>,
    pub git: GitState,
    pub depth: usize,
}

#[derive(Debug, Clone)]
pub struct FlatEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub expanded: bool,
    pub git: GitState,
    pub depth: usize,
    /// Whether this node is the last sibling of its parent.
    pub last: bool,
    /// For each ancestor: whether that ancestor was the last sibling.
    pub guides: Vec<bool>,
}

fn excluded(name: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|p| {
        if let Some(suffix) = p.strip_prefix('*') {
            name.ends_with(suffix)
        } else {
            name == p
        }
    })
}

impl FileNode {
    pub fn build(root: &Path, excludes: &[String], git: &GitInfo, depth: usize) -> Self {
        let name = root
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| root.to_string_lossy().to_string());
        let mut node = Self {
            path: root.to_path_buf(),
            name,
            is_dir: root.is_dir(),
            expanded: depth == 0,
            children: Vec::new(),
            git: git.changes.get(root).copied().unwrap_or(GitState::Clean),
            depth,
        };
        if !node.is_dir || depth > 64 {
            return node;
        }
        let mut entries: Vec<_> = match std::fs::read_dir(root) {
            Ok(read) => read.filter_map(Result::ok).collect(),
            Err(_) => return node,
        };
        entries.sort_by_key(|e| {
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            (!is_dir, e.file_name().to_string_lossy().to_lowercase())
        });
        for entry in entries {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if excluded(&file_name, excludes) {
                continue;
            }
            let path = entry.path();
            if path.is_dir() || path.is_file() {
                node.children.push(Self::build(&path, excludes, git, depth + 1));
            }
        }
        node
    }

    pub fn flatten(&self, depth: usize, out: &mut Vec<FlatEntry>) {
        self.flatten_with(depth, &mut Vec::new(), out);
    }

    fn flatten_with(&self, depth: usize, ancestors_last: &mut Vec<bool>, out: &mut Vec<FlatEntry>) {
        for (index, child) in self.children.iter().enumerate() {
            let last = index + 1 == self.children.len();
            out.push(FlatEntry {
                path: child.path.clone(),
                name: child.name.clone(),
                is_dir: child.is_dir,
                expanded: child.expanded,
                git: child.git,
                depth,
                last,
                guides: ancestors_last.clone(),
            });
            if child.is_dir && child.expanded {
                ancestors_last.push(last);
                child.flatten_with(depth + 1, ancestors_last, out);
                ancestors_last.pop();
            }
        }
    }

    pub fn find_mut(&mut self, path: &Path) -> Option<&mut FileNode> {
        if self.path == path {
            return Some(self);
        }
        for child in &mut self.children {
            if let Some(found) = child.find_mut(path) {
                return Some(found);
            }
        }
        None
    }

    pub fn collect_expanded(&self) -> HashSet<PathBuf> {
        let mut set = HashSet::new();
        fn walk(node: &FileNode, set: &mut HashSet<PathBuf>) {
            if node.is_dir && node.expanded {
                set.insert(node.path.clone());
            }
            for child in &node.children {
                walk(child, set);
            }
        }
        walk(self, &mut set);
        set
    }

    pub fn restore_expanded(&mut self, set: &HashSet<PathBuf>) {
        if self.is_dir {
            self.expanded = set.contains(&self.path) || self.depth == 0;
        }
        for child in &mut self.children {
            child.restore_expanded(set);
        }
    }

    pub fn icon(&self) -> &'static str {
        if self.is_dir { "" } else { Language::detect(&self.path).file_icon() }
    }

    pub fn git_label(&self) -> &'static str {
        match self.git {
            GitState::Modified => "M",
            GitState::Added | GitState::Untracked => "A",
            GitState::Deleted => "D",
            GitState::Conflict => "!",
            GitState::Ignored => "I",
            GitState::Clean => " ",
        }
    }
}

/// Expand every ancestor of `path` so it becomes visible in the explorer.
pub fn reveal_path(tree: &mut FileNode, path: &Path) {
    if tree.path == path { return; }
    for child in &mut tree.children {
        if path.starts_with(&child.path) {
            if child.is_dir {
                child.expanded = true;
            }
            reveal_path(child, path);
            return;
        }
    }
}
