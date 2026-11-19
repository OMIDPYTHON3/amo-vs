pub mod filetree;
pub mod search;
pub mod watcher;

pub use filetree::{reveal_path, FileNode, FlatEntry};
pub use search::SearchResult;
pub use watcher::Watcher;

use crate::services::git::{self, GitInfo};
use crate::services::Config;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitState {
    Clean,
    Modified,
    Added,
    Deleted,
    Untracked,
    Ignored,
    Conflict,
}

/// An open workspace root with its file tree and git status.
pub struct Workspace {
    pub root: PathBuf,
    pub tree: FileNode,
    pub selected: Option<PathBuf>,
    pub git: GitInfo,
}

impl Workspace {
    pub fn open(path: &Path, config: &Config) -> io::Result<Self> {
        let root = if path.is_file() {
            path.parent().unwrap_or(Path::new(".")).to_path_buf()
        } else {
            path.to_path_buf()
        };
        let root = std::fs::canonicalize(&root).unwrap_or(root);
        let git_info = if config.git_enabled {
            git::status(git::repo_root(&root).as_deref().unwrap_or(&root))
        } else {
            GitInfo::default()
        };
        let tree = FileNode::build(&root, &config.exclude, &git_info, 0);
        Ok(Self { root, tree, selected: None, git: git_info })
    }

    pub fn refresh(&mut self, config: &Config) {
        if config.git_enabled {
            self.git = git::status(git::repo_root(&self.root).as_deref().unwrap_or(&self.root));
        }
        let expanded = self.tree.collect_expanded();
        let selected = self.selected.clone();
        self.tree = FileNode::build(&self.root, &config.exclude, &self.git, 0);
        self.tree.restore_expanded(&expanded);
        self.selected = selected;
    }

    pub fn flatten(&self) -> Vec<FlatEntry> {
        let mut out = Vec::new();
        self.tree.flatten(0, &mut out);
        out
    }

    pub fn toggle(&mut self, path: &Path) {
        if let Some(node) = self.tree.find_mut(path) {
            if node.is_dir {
                node.expanded = !node.expanded;
            }
        }
    }

    pub fn select(&mut self, path: PathBuf) {
        self.selected = Some(path);
    }

    pub fn git_state(&self, path: &Path) -> GitState {
        self.git.changes.get(path).copied().unwrap_or(GitState::Clean)
    }

    pub fn create_file(&mut self, path: &Path, config: &Config) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, "")?;
        self.refresh(config);
        Ok(())
    }

    pub fn create_folder(&mut self, path: &Path, config: &Config) -> io::Result<()> {
        std::fs::create_dir_all(path)?;
        self.refresh(config);
        Ok(())
    }

    pub fn rename(&mut self, from: &Path, to: &Path, config: &Config) -> io::Result<()> {
        std::fs::rename(from, to)?;
        self.refresh(config);
        Ok(())
    }

    pub fn delete(&mut self, path: &Path, config: &Config) -> io::Result<()> {
        if path.is_dir() {
            std::fs::remove_dir_all(path)?;
        } else {
            std::fs::remove_file(path)?;
        }
        self.refresh(config);
        Ok(())
    }
}
