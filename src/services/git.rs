use crate::workspace::GitState;
use git2::{Repository, Status, StatusOptions};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub struct GitInfo {
    pub changes: HashMap<PathBuf, GitState>,
    pub branch: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub root: Option<PathBuf>,
}

/// Discover the repository root for a path by walking up the directory tree.
pub fn repo_root(path: &Path) -> Option<PathBuf> {
    let start = if path.is_file() { path.parent()? } else { path };
    Repository::discover(start)
        .ok()
        .and_then(|r| r.workdir().map(|p| p.to_path_buf()))
}

fn map_status(s: Status) -> GitState {
    if s.contains(Status::CONFLICTED) { GitState::Conflict }
    else if s.contains(Status::WT_DELETED) || s.contains(Status::INDEX_DELETED) { GitState::Deleted }
    else if s.contains(Status::WT_NEW) || s.contains(Status::INDEX_NEW) { GitState::Added }
    else if s.contains(Status::WT_MODIFIED) || s.contains(Status::INDEX_MODIFIED) || s.contains(Status::WT_TYPECHANGE) { GitState::Modified }
    else if s.contains(Status::IGNORED) { GitState::Ignored }
    else { GitState::Untracked }
}

/// Collect working-tree status. Local-only: no network access is performed.
pub fn status(root: &Path) -> GitInfo {
    let mut info = GitInfo { root: Some(root.to_path_buf()), ..Default::default() };
    let repo = match Repository::open(root) {
        Ok(r) => r,
        Err(_) => return info,
    };

    if let Ok(head) = repo.head() {
        if head.is_branch() {
            info.branch = head.shorthand().map(|s| s.to_string());
        }
        if let Some(branch_name) = head.shorthand() {
            if let Ok(branch) = repo.find_branch(branch_name, git2::BranchType::Local) {
                if let Ok(upstream) = branch.upstream() {
                    let local_oid = branch.get().target();
                    let upstream_oid = upstream.get().target();
                    if let (Some(l), Some(u)) = (local_oid, upstream_oid) {
                        if let Ok((ahead, behind)) = repo.graph_ahead_behind(l, u) {
                            info.ahead = ahead;
                            info.behind = behind;
                        }
                    }
                }
            }
        }
    }

    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .include_ignored(false)
        .recurse_untracked_dirs(true)
        .include_unmodified(false);

    if let Ok(statuses) = repo.statuses(Some(&mut opts)) {
        for entry in statuses.iter() {
            if let Some(relative) = entry.path() {
                let path = root.join(relative);
                let state = map_status(entry.status());
                info.changes.insert(path, state);
            }
        }
    }
    info
}

/// Stage all working-tree changes and create a commit.
pub fn commit(root: &Path, message: &str) -> Result<(), String> {
    let repo = Repository::open(root).map_err(|e| e.to_string())?;
    let mut index = repo.index().map_err(|e| e.to_string())?;
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .map_err(|e| e.to_string())?;
    index.write().map_err(|e| e.to_string())?;
    let tree_id = index.write_tree().map_err(|e| e.to_string())?;
    let tree = repo.find_tree(tree_id).map_err(|e| e.to_string())?;
    let signature = repo.signature().map_err(|_e| {
        "Set user.name and user.email in git config before committing".to_string()
    })?;
    let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    let parents: Vec<&git2::Commit> = parent.iter().collect();
    repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &parents)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Short status label used for file decorations (M, A, D, U).
pub fn short_label(state: GitState) -> &'static str {
    match state {
        GitState::Modified => "M",
        GitState::Added => "A",
        GitState::Deleted => "D",
        GitState::Conflict => "!",
        GitState::Untracked => "U",
        GitState::Ignored => "I",
        GitState::Clean => " ",
    }
}
