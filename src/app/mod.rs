pub mod action;
pub mod command;
pub mod events;
pub mod state;

pub use action::Action;
pub use state::*;

use crate::editor::Language;
use crate::workspace::{reveal_path, Workspace};
use std::io;
use std::path::{Path, PathBuf};

impl App {
    // ---- Completion -----------------------------------------------------

    pub fn update_completion(&mut self) {
        let config = self.config.clone();
        if !config.autocomplete { self.completion.clear(); return; }
        let root = self.workspace.as_ref().map(|w| w.root.as_path());
        let Some(editor) = self.editors.get(self.active) else { self.completion.clear(); return; };
        let cursor = editor.view.cursor;
        let byte = editor.buffer.char_to_byte(cursor).saturating_sub(1);
        let context = editor.highlighter.context_at_byte(byte);
        let state = self.completion_engine.compute(&editor.buffer, cursor, root, &config, context);
        self.completion = state.unwrap_or_default();
    }

    pub fn dismiss_completion(&mut self) { self.completion.clear(); }

    pub fn completion_move(&mut self, delta: isize) {
        self.completion.move_selection(delta);
    }

    pub fn accept_completion(&mut self) {
        let Some(item) = self.completion.accept().cloned() else { return; };
        let start = self.completion.replace_start;
        let end = self.completion.replace_end;
        let inserted = item.insert_text.chars().count();
        if let Some(editor) = self.active_editor_mut() {
            if end > start { editor.buffer.remove(start, end - start, false); }
            editor.buffer.insert(start, &item.insert_text, false);
            editor.view.cursor = start + inserted;
            editor.view.anchor = None;
            editor.view.desired_col = editor.view.column(&editor.buffer);
        }
        self.completion.clear();
    }

    // ---- Theme picker ---------------------------------------------------

    pub fn open_theme_picker(&mut self) {
        let names: Vec<String> = crate::themes::BUILTIN_NAMES.iter().map(|s| s.to_string()).collect();
        let selected = names.iter()
            .position(|n| n.eq_ignore_ascii_case(&self.config.theme))
            .unwrap_or(0);
        self.theme_picker = Some(ThemePickerState {
            query: String::new(),
            selected,
            names,
            original: self.theme.clone(),
            original_index: selected,
        });
        self.overlay = Some(Overlay::ThemePicker);
        self.update_theme_picker();
    }

    /// Filter the list and live-preview the highlighted theme.
    pub fn update_theme_picker(&mut self) {
        let Some(picker) = self.theme_picker.as_mut() else { return; };
        let query = picker.query.to_lowercase();
        let all = picker.names.clone();
        picker.names = all.into_iter()
            .filter(|n| query.is_empty() || n.to_lowercase().contains(&query))
            .collect();
        if picker.names.is_empty() {
            picker.selected = 0;
            return;
        }
        picker.selected = picker.selected.min(picker.names.len() - 1);
        if let Some(name) = picker.names.get(picker.selected).cloned() {
            if let Some(theme) = crate::themes::builtin(&name) {
                self.theme = theme;
            }
        }
    }

    pub fn theme_picker_move(&mut self, delta: isize) {
        let Some(picker) = self.theme_picker.as_ref() else { return; };
        if picker.names.is_empty() { return; }
        let count = picker.names.len();
        let sel = picker.selected as isize + delta;
        let new = ((sel % count as isize) + count as isize) as usize % count;
        self.theme_picker.as_mut().unwrap().selected = new;
        self.update_theme_picker();
    }

    pub fn theme_picker_accept(&mut self) {
        let Some(picker) = self.theme_picker.as_ref() else { return; };
        let Some(name) = picker.names.get(picker.selected).cloned() else { return; };
        if crate::themes::apply(&mut self.theme, &mut self.config, &name) {
            self.notify(format!("Theme: {}", name), MessageKind::Info);
        }
        self.theme_picker = None;
        self.overlay = None;
    }

    pub fn theme_picker_cancel(&mut self) {
        if let Some(picker) = self.theme_picker.take() {
            self.theme = picker.original;
        }
        self.overlay = None;
    }

    pub fn cycle_theme(&mut self, delta: isize) {
        let names = crate::themes::BUILTIN_NAMES;
        let current = names.iter().position(|n| n.eq_ignore_ascii_case(&self.config.theme)).unwrap_or(0);
        let next = ((current as isize + delta) % names.len() as isize + names.len() as isize) as usize % names.len();
        let name = names[next].to_string();
        if crate::themes::apply(&mut self.theme, &mut self.config, &name) {
            self.notify(format!("Theme: {}", name), MessageKind::Info);
        }
    }

    /// Handle the path supplied on the command line.
    pub fn open_startup(&mut self, path: &Path) {
        if path.is_dir() {
            if let Err(e) = self.open_workspace(path.to_path_buf()) {
                self.notify(format!("Could not open folder: {}", e), MessageKind::Error);
            }
        } else if path.is_file() {
            if self.open_path(path).is_ok() && self.active > 0 {
                let welcome_is_scratch = self.editors[0].path().is_none() && !self.editors[0].dirty();
                if welcome_is_scratch {
                    self.editors.remove(0);
                    self.active -= 1;
                }
            }
        }
    }

    /// Open an in-app file browser (no native dialog dependency).
    pub fn open_file_browser(&mut self, mode: FileBrowserMode) {
        let dir = self.cwd();
        let mut browser = FileBrowserState {
            dir: dir.clone(),
            entries: Vec::new(),
            selected: 0,
            filter: String::new(),
            mode,
        };
        refresh_browser(&mut browser);
        self.file_browser = Some(browser);
        self.overlay = Some(Overlay::FileBrowser);
    }

    /// Open the explorer/file context menu at the given screen position.
    pub fn open_context_menu(&mut self, path: PathBuf, is_dir: bool, x: u16, y: u16) {
        let mut actions = vec![ContextAction::Open];
        if is_dir {
            actions.push(ContextAction::NewFile);
            actions.push(ContextAction::NewFolder);
        }
        actions.extend([ContextAction::Rename, ContextAction::Delete, ContextAction::CopyPath]);
        if !is_dir { actions.push(ContextAction::Reveal); }
        self.context_menu = Some(ContextMenuState {
            path, is_dir, actions, selected: 0, x, y,
        });
        self.overlay = Some(Overlay::ContextMenu);
        self.focus = Focus::Explorer;
    }

    /// Execute the currently highlighted context-menu action.
    pub fn execute_context_action(&mut self) {
        let Some(menu) = self.context_menu.as_ref() else { return; };
        let action = menu.actions.get(menu.selected).copied();
        let path = menu.path.clone();
        let is_dir = menu.is_dir;
        self.context_menu = None;
        self.overlay = None;
        let Some(action) = action else { return; };
        match action {
            ContextAction::Open => {
                if is_dir {
                    if let Some(workspace) = &mut self.workspace { workspace.toggle(&path); }
                } else if let Err(e) = self.open_path(&path) {
                    self.notify(format!("Open failed: {}", e), MessageKind::Error);
                }
            }
            ContextAction::NewFile => {
                self.open_prompt(PromptKind::NewFile);
                self.cwd_for_prompt = Some(path);
            }
            ContextAction::NewFolder => {
                self.open_prompt(PromptKind::NewFolder);
                self.cwd_for_prompt = Some(path);
            }
            ContextAction::Rename => {
                self.open_prompt(PromptKind::Rename);
                if let Some(prompt) = &mut self.prompt { prompt.target = Some(path); }
            }
            ContextAction::Delete => self.execute(Action::DeletePath(path)),
            ContextAction::CopyPath => {
                self.clipboard.set(path.to_string_lossy().to_string());
                self.notify("Path copied", MessageKind::Info);
            }
            ContextAction::Reveal => self.execute(Action::ExplorerRevealActive),
        }
    }

    /// Central exit entry point. Every exit path (shortcut, menu, dialog)
    /// funnels through here so unsaved changes are never bypassed.
    pub fn request_exit(&mut self) {
        if self.exit_requested { return; } // a single keypress exits at most once
        self.exit_requested = true;
        self.exit_snapshot = Some(UiSnapshot::capture(self));
        if self.editors.iter().any(|e| e.dirty()) {
            self.confirm = Some(ConfirmState::exit());
            self.overlay = Some(Overlay::Confirm);
        } else {
            self.quit = true;
        }
    }

    /// Save every modified file; returns the number saved or the first error.
    pub fn save_all_result(&mut self) -> Result<usize, String> {
        let mut saved = 0usize;
        for editor in &mut self.editors {
            if !editor.dirty() { continue; }
            if editor.path().is_none() {
                return Err(format!("\"{}\" has no file path - use Save As first", editor.name()));
            }
            editor.save().map_err(|e| format!("{}: {}", editor.name(), e))?;
            saved += 1;
        }
        Ok(saved)
    }

    pub fn confirm_save_all_and_exit(&mut self) {
        match self.save_all_result() {
            Ok(count) => {
                self.confirm = None;
                self.overlay = None;
                self.notify(format!("Saved {} file(s)", count), MessageKind::Info);
                self.quit = true;
            }
            Err(error) => {
                // Stay in the application and restore the exact UI state.
                self.confirm = None;
                self.exit_requested = false;
                if let Some(snapshot) = self.exit_snapshot.take() { snapshot.restore(self); }
                self.notify(format!("Cannot exit: {}", error), MessageKind::Error);
            }
        }
    }

    /// Release background resources (PTY sessions) before restoring the terminal.
    /// Returns the PIDs of shells that were terminated, for verification.
    pub fn shutdown(&mut self) -> Vec<u32> {
        let pids = self.terminal.close_all();
        self.log(format!("Shut down {} terminal session(s)", pids.len()));
        pids
    }

    pub fn confirm_accept(&mut self) {
        let Some(confirm) = self.confirm.take() else { return; };
        if confirm.style == ConfirmStyle::Exit {
            self.confirm_save_all_and_exit();
            return;
        }
        self.overlay = None;
        match confirm.action {
            PendingAction::Delete(path) => {
                if let Some(workspace) = &mut self.workspace {
                    match workspace.delete(&path, &self.config) {
                        Ok(()) => self.notify(format!("Deleted {}", path.display()), MessageKind::Info),
                        Err(e) => self.notify(format!("Delete failed: {}", e), MessageKind::Error),
                    }
                }
                for i in (0..self.editors.len()).rev() {
                    if self.editors[i].path() == Some(path.as_path()) {
                        self.editors.remove(i);
                    }
                }
                if self.editors.is_empty() { self.new_untitled(); }
                self.active = self.active.min(self.editors.len().saturating_sub(1));
            }
            PendingAction::CloseTab(index) => {
                if index < self.editors.len() {
                    self.editors.remove(index);
                    if index < self.active { self.active = self.active.saturating_sub(1); }
                }
                self.active = self.active.min(self.editors.len().saturating_sub(1));
                if self.editors.is_empty() { self.new_untitled(); }
            }
            PendingAction::Reload(index) => {
                if let Some(editor) = self.editors.get_mut(index) {
                    if editor.reload().is_ok() { self.notify("File reverted", MessageKind::Info); }
                }
            }
            PendingAction::Quit => self.quit = true,
        }
    }

    pub fn prompt_accept(&mut self) {
        let Some(prompt) = self.prompt.take() else { return; };
        self.overlay = None;
        let value = prompt.value.trim().to_string();
        match prompt.kind {
            PromptKind::NewFile | PromptKind::NewFolder => {
                if value.is_empty() { return; }
                let base = self.cwd_for_prompt.take().unwrap_or_else(|| self.cwd());
                let target = resolve_input(&base, &value);
                let workspace = self.workspace.as_mut();
                let result = if prompt.kind == PromptKind::NewFile {
                    workspace.map(|w| w.create_file(&target, &self.config))
                } else {
                    workspace.map(|w| w.create_folder(&target, &self.config))
                };
                match result {
                    Some(Ok(())) => {
                        if prompt.kind == PromptKind::NewFile {
                            if self.open_path(&target).is_ok() { self.notify("File created", MessageKind::Info); }
                        } else {
                            self.notify("Folder created", MessageKind::Info);
                        }
                    }
                    Some(Err(e)) => self.notify(format!("Failed: {}", e), MessageKind::Error),
                    None => self.notify("Open a folder first", MessageKind::Warning),
                }
            }
            PromptKind::Rename => {
                let Some(from) = prompt.target else { return; };
                if value.is_empty() { return; }
                let to = from.parent().unwrap_or(Path::new(".")).join(&value);
                if let Some(workspace) = &mut self.workspace {
                    match workspace.rename(&from, &to, &self.config) {
                        Ok(()) => {
                            for editor in &mut self.editors {
                                if editor.path() == Some(from.as_path()) {
                                    let _ = editor.save_as(to.clone());
                                }
                            }
                            self.notify("Renamed", MessageKind::Info);
                        }
                        Err(e) => self.notify(format!("Rename failed: {}", e), MessageKind::Error),
                    }
                }
            }
            PromptKind::SaveAs => {
                if value.is_empty() { return; }
                let target = resolve_input(&self.cwd(), &value);
                if let Some(editor) = self.active_editor_mut() {
                    match editor.save_as(target.clone()) {
                        Ok(()) => {
                            self.notify(format!("Saved {}", target.display()), MessageKind::Info);
                            self.refresh_git();
                        }
                        Err(e) => self.notify(format!("Save failed: {}", e), MessageKind::Error),
                    }
                }
            }
            PromptKind::GitCommit => {
                if value.is_empty() { return; }
                let root = self.workspace.as_ref().and_then(|w| w.git.root.clone());
                match root {
                    Some(root) => match crate::services::git::commit(&root, &value) {
                        Ok(()) => {
                            self.notify("Commit created", MessageKind::Info);
                            self.refresh_git();
                        }
                        Err(e) => self.notify(format!("Commit failed: {}", e), MessageKind::Error),
                    },
                    None => self.notify("No git repository in this workspace", MessageKind::Warning),
                }
            }
            PromptKind::GotoLine => {
                let num: usize = match value.trim().parse() {
                    Ok(n) => n,
                    Err(_) => {
                        self.notify("Enter a valid line number", MessageKind::Warning);
                        return;
                    }
                };
                if let Some(editor) = self.active_editor_mut() {
                    let line = num.saturating_sub(1).min(editor.buffer.len_lines().saturating_sub(1));
                    editor.view.cursor = editor.buffer.line_to_char(line);
                    editor.view.anchor = None;
                    editor.view.desired_col = 0;
                }
            }
        }
    }

    pub fn browser_enter(&mut self) {
        let Some(browser) = self.file_browser.as_mut() else { return; };
        if let Some((path, _name, is_dir)) = browser.entries.get(browser.selected).cloned() {
            if is_dir {
                browser.dir = path;
                browser.selected = 0;
                browser.filter.clear();
                refresh_browser(browser);
                return;
            }
            if browser.mode == FileBrowserMode::OpenFolder {
                browser.dir = path.parent().map(|p| p.to_path_buf()).unwrap_or(path);
                browser.filter.clear();
                refresh_browser(browser);
                return;
            }
            let path = path.clone();
            self.file_browser = None;
            self.overlay = None;
            if let Err(e) = self.open_path(&path) {
                self.notify(format!("Open failed: {}", e), MessageKind::Error);
            }
            return;
        }
        // Empty selection: interpret input as a path.
        let input = browser.filter.clone();
        let dir = browser.dir.clone();
        let target = resolve_input(&dir, &input);
        if browser.mode == FileBrowserMode::OpenFolder && target.is_dir() {
            if let Err(e) = self.open_workspace(target) {
                self.notify(format!("Open folder failed: {}", e), MessageKind::Error);
            }
        } else if target.is_file() {
            let _ = self.open_path(&target);
        }
        self.file_browser = None;
        self.overlay = None;
    }

    pub fn browser_up(&mut self) {
        if let Some(browser) = self.file_browser.as_mut() {
            if let Some(parent) = browser.dir.parent() {
                browser.dir = parent.to_path_buf();
                browser.selected = 0;
                browser.filter.clear();
                refresh_browser(browser);
            }
        }
    }

    pub fn open_workspace(&mut self, path: PathBuf) -> io::Result<()> {
        let workspace = Workspace::open(&path, &self.config)?;
        crate::platform::remember_project(&workspace.root, 10);
        // Update in-memory state directly so it is deterministic even if the
        // shared history file is being written by another process.
        self.recent_projects.retain(|p| p != &workspace.root);
        self.recent_projects.insert(0, workspace.root.clone());
        self.recent_projects.truncate(8);
        self.workspace = Some(workspace);
        self.editors.clear();
        self.active = 0;
        self.next_id = 1;
        self.open_welcome();
        self.explorer_selected = 0;
        self.explorer_scroll = 0;
        self.diagnostics.clear();
        self.notify(format!("Workspace: {}", path.display()), MessageKind::Info);
        Ok(())
    }

    pub fn run_active_file(&mut self) {
        let Some(editor) = self.active_editor() else { return; };
        let Some(path) = editor.path().map(|p| p.to_path_buf()) else {
            self.notify("Save the file before running it", MessageKind::Warning);
            return;
        };
        let quoted = shell_quote(&path.to_string_lossy());
        let command = match editor.buffer.language {
            Language::Python => format!("python3 {}", quoted),
            Language::JavaScript => format!("node {}", quoted),
            Language::Shell => format!("bash {}", quoted),
            Language::Go => format!("go run {}", quoted),
            Language::Rust => {
                let manifest = find_cargo_manifest(&path);
                match manifest {
                    Some(m) => format!("cargo run --manifest-path {}", shell_quote(&m.to_string_lossy())),
                    None => {
                        let out = std::env::temp_dir().join("amo-vs-run");
                        format!("rustc {} -o {} && {}", quoted, shell_quote(&out.to_string_lossy()), shell_quote(&out.to_string_lossy()))
                    }
                }
            }
            Language::Java => {
                let class = path.with_extension("");
                format!("javac {} && java {}", quoted, shell_quote(&class.to_string_lossy()))
            }
            _ => {
                self.notify("No runner configured for this language", MessageKind::Warning);
                return;
            }
        };
        self.show_terminal();
        let cwd = self.cwd();
        if self.terminal.ensure_session(&cwd).is_err() {
            self.notify("Could not start a terminal", MessageKind::Error);
            return;
        }
        self.focus = Focus::Terminal;
        if let Some(session) = self.terminal.active_mut() {
            session.input.clear();
            session.write_bytes(command.as_bytes());
            session.submit(self.config.terminal_history_size);
        }
    }

    pub fn show_terminal(&mut self) {
        self.panel.visible = true;
        self.panel.tab = PanelTab::Terminal;
        self.sync_terminal_size();
        if self.terminal.sessions.is_empty() {
            let cwd = self.cwd();
            if let Err(e) = self.terminal.spawn(&cwd) {
                self.notify(format!("Terminal failed: {}", e), MessageKind::Error);
            }
        }
    }

    pub fn terminal_height(&self) -> usize { self.terminal.height.max(4) }
    pub fn terminal_width(&self) -> usize { self.terminal.width.max(20) }

    pub fn set_terminal_size(&mut self, cols: usize, rows: usize) {
        self.terminal_size = (cols as u16, rows as u16);
        self.sync_terminal_size();
    }

    pub fn sidebar_x(&self) -> u16 {
        crate::ui::layout::compute(self, ratatui::layout::Rect {
            x: 0, y: 0, width: self.terminal_size.0, height: self.terminal_size.1,
        }).sidebar.x
    }

    pub fn sidebar_y(&self) -> u16 {
        crate::ui::layout::compute(self, ratatui::layout::Rect {
            x: 0, y: 0, width: self.terminal_size.0, height: self.terminal_size.1,
        }).sidebar.y
    }

    /// Match the PTY dimensions to the exact panel area so shell programs
    /// wrap at the right column and the prompt stays visible.
    pub fn sync_terminal_size(&mut self) {
        let (w, h) = self.terminal_size;
        if w == 0 || h == 0 { return; }
        // Reuse the exact same layout code as rendering; this guarantees the
        // PTY dimensions match the visible panel on every screen size.
        let r = crate::ui::layout::compute(
            self,
            ratatui::layout::Rect { x: 0, y: 0, width: w, height: h },
        );
        if r.panel_body.width == 0 || r.panel_body.height == 0 { return; }
        self.terminal.resize(r.panel_body.width as usize, r.panel_body.height as usize);
    }

    // ---- Command dispatcher ---------------------------------------------

    pub fn execute(&mut self, action: Action) {
        let config = self.config.clone();
        match action {
            Action::NewFile => self.new_untitled(),
            Action::OpenFile => self.open_file_browser(FileBrowserMode::OpenFile),
            Action::OpenFolder => self.open_file_browser(FileBrowserMode::OpenFolder),
            Action::Save => self.save(),
            Action::SaveAs => self.open_prompt(PromptKind::SaveAs),
            Action::SaveAll => self.save_all(),
            Action::CloseTab => {
                let index = self.active;
                self.close_tab(index);
            }
            Action::CloseOtherTabs => {
                let keep = self.active;
                let mut removed = 0;
                let mut index = 0;
                while index < self.editors.len() {
                    if index != keep && !self.editors[index].dirty() {
                        self.editors.remove(index);
                        removed += 1;
                    } else {
                        index += 1;
                    }
                }
                self.active = self.active.min(self.editors.len().saturating_sub(1));
                self.notify(format!("Closed {} editor(s)", removed), MessageKind::Info);
            }
            Action::Quit => self.request_exit(),
            Action::SaveAllAndExit => self.confirm_save_all_and_exit(),
            Action::ExitWithoutSaving => {
                self.confirm = None;
                self.overlay = None;
                self.quit = true;
            }
            Action::CancelExit => {
                self.confirm = None;
                self.exit_requested = false;
                if let Some(snapshot) = self.exit_snapshot.take() { snapshot.restore(self); }
            }
            Action::Undo => {
                if let Some(editor) = self.active_editor_mut() {
                    if let Some(pos) = editor.buffer.undo() {
                        editor.view.cursor = pos.min(editor.buffer.len_chars());
                        editor.view.anchor = None;
                    }
                }
            }
            Action::Redo => {
                if let Some(editor) = self.active_editor_mut() {
                    if let Some(pos) = editor.buffer.redo() {
                        editor.view.cursor = pos.min(editor.buffer.len_chars());
                        editor.view.anchor = None;
                    }
                }
            }
            Action::Copy => {
                let text = self.active_editor().and_then(|e| e.view.selection())
                    .map(|(s, e)| self.active_editor().unwrap().buffer.slice(s, e))
                    .unwrap_or_default();
                self.clipboard.set(text);
            }
            Action::Cut => {
                let (text, range) = self.active_editor().and_then(|e| e.view.selection().map(|r| (r, e.buffer.slice(r.0, r.1))))
                    .map(|(r, t)| (t, r))
                    .unwrap_or_default();
                if range.0 != range.1 && !text.is_empty() {
                    self.clipboard.set(text);
                    if let Some(editor) = self.active_editor_mut() {
                        editor.buffer.remove(range.0, range.1 - range.0, false);
                        editor.view.cursor = range.0;
                        editor.view.anchor = None;
                    }
                }
            }
            Action::Paste => {
                let text = self.clipboard.get();
                if !text.is_empty() {
                    if let Some(editor) = self.active_editor_mut() {
                        editor.insert_text(&text, &config);
                    }
                }
            }
            Action::SelectAll => {
                if let Some(editor) = self.active_editor_mut() {
                    editor.view.select_all(&editor.buffer);
                }
            }
            Action::Find | Action::Replace => {
                if let Some(editor) = self.active_editor() {
                    if let Some((s, e)) = editor.view.selection() {
                        let selected = editor.buffer.slice(s, e);
                        if !selected.contains('\n') && !selected.is_empty() {
                            self.find.query = selected;
                        }
                    }
                }
                self.overlay = Some(if action == Action::Replace { Overlay::Replace } else { Overlay::Find });
                self.find_in_editor();
            }
            Action::FindNext => {
                self.find_in_editor();
                let next = if self.find.current + 1 < self.find.matches.len() { self.find.current + 1 } else { 0 };
                self.goto_match(next);
            }
            Action::FindPrevious => {
                self.find_in_editor();
                let prev = if self.find.current == 0 {
                    self.find.matches.len().saturating_sub(1)
                } else { self.find.current - 1 };
                self.goto_match(prev);
            }
            Action::ToggleComment => {
                if let Some(editor) = self.active_editor_mut() { editor.toggle_comment(&config); }
            }
            Action::Indent => {
                if let Some(editor) = self.active_editor_mut() { editor.indent(&config); }
            }
            Action::Outdent => {
                if let Some(editor) = self.active_editor_mut() { editor.outdent(&config); }
            }
            Action::DuplicateLine => {
                if let Some(editor) = self.active_editor_mut() {
                    let line = editor.buffer.char_to_line(editor.view.cursor);
                    let start = editor.buffer.line_to_char(line);
                    let end = start + editor.buffer.line_content_len(line);
                    let text = editor.buffer.slice(start, end);
                    let insert = format!("\n{}", text);
                    editor.buffer.insert(end, &insert, false);
                    editor.view.cursor = end + 1 + text.chars().count();
                }
            }
            Action::DeleteLine => {
                if let Some(editor) = self.active_editor_mut() {
                    let line = editor.buffer.char_to_line(editor.view.cursor);
                    let start = editor.buffer.line_to_char(line);
                    let next = editor.buffer.line_to_char((line + 1).min(editor.buffer.len_lines() - 1));
                    let end = if line + 1 < editor.buffer.len_lines() { next } else { editor.buffer.len_chars() };
                    editor.buffer.remove(start, end - start, false);
                    editor.view.cursor = start.min(editor.buffer.len_chars());
                }
            }
            Action::GotoLine => self.open_prompt(PromptKind::GotoLine),
            Action::CursorLeft => { if let Some(e) = self.active_editor_mut() { e.view.move_left(&e.buffer, false); } }
            Action::CursorRight => { if let Some(e) = self.active_editor_mut() { e.view.move_right(&e.buffer, false); } }
            Action::CursorUp => { if let Some(e) = self.active_editor_mut() { e.view.move_up(&e.buffer, false); } }
            Action::CursorDown => { if let Some(e) = self.active_editor_mut() { e.view.move_down(&e.buffer, false); } }
            Action::WordLeft => { if let Some(e) = self.active_editor_mut() { e.view.move_word_left(&e.buffer, false); } }
            Action::WordRight => { if let Some(e) = self.active_editor_mut() { e.view.move_word_right(&e.buffer, false); } }
            Action::CursorHome => { if let Some(e) = self.active_editor_mut() { e.view.move_home(&e.buffer, false); } }
            Action::CursorEnd => { if let Some(e) = self.active_editor_mut() { e.view.move_end(&e.buffer, false); } }
            Action::DocStart => { if let Some(e) = self.active_editor_mut() { e.view.move_doc_start(false); } }
            Action::DocEnd => { if let Some(e) = self.active_editor_mut() { e.view.move_doc_end(&e.buffer, false); } }
            Action::PageUp => { if let Some(e) = self.active_editor_mut() { e.view.move_page_up(&e.buffer, 20, false); } }
            Action::PageDown => { if let Some(e) = self.active_editor_mut() { e.view.move_page_down(&e.buffer, 20, false); } }
            Action::ToggleSidebar => self.sidebar_visible = !self.sidebar_visible,
            Action::TogglePanel => {
                self.panel.visible = !self.panel.visible;
                if self.panel.visible { self.show_terminal(); }
            }
            Action::ToggleTerminal => {
                if self.panel.visible && self.panel.tab == PanelTab::Terminal {
                    self.panel.visible = false;
                } else {
                    self.show_terminal();
                    self.focus = Focus::Terminal;
                }
            }
            Action::ToggleStatusBar => self.config.status_bar_visible = !self.config.status_bar_visible,
            Action::ToggleWordWrap => self.config.word_wrap = !self.config.word_wrap,
            Action::ToggleLineNumbers => self.config.show_line_numbers = !self.config.show_line_numbers,
            Action::FocusExplorer => {
                self.focus = Focus::Explorer;
                self.sidebar_visible = true;
                self.sidebar_view = SideBarView::Explorer;
            }
            Action::FocusEditor => self.focus = Focus::Editor,
            Action::FocusTerminal => { self.show_terminal(); self.focus = Focus::Terminal; }
            Action::NextTab => {
                if !self.editors.is_empty() { self.active = (self.active + 1) % self.editors.len(); }
                self.focus = Focus::Editor;
            }
            Action::PreviousTab => {
                if !self.editors.is_empty() {
                    self.active = (self.active + self.editors.len() - 1) % self.editors.len();
                }
                self.focus = Focus::Editor;
            }
            Action::NextPanelTab => {
                let tabs = PanelTab::all();
                let i = tabs.iter().position(|t| *t == self.panel.tab).unwrap_or(0);
                self.panel.tab = tabs[(i + 1) % tabs.len()];
                self.panel.visible = true;
            }
            Action::PreviousPanelTab => {
                let tabs = PanelTab::all();
                let i = tabs.iter().position(|t| *t == self.panel.tab).unwrap_or(0);
                self.panel.tab = tabs[(i + tabs.len() - 1) % tabs.len()];
                self.panel.visible = true;
            }
            Action::CommandPalette => self.open_palette(false),
            Action::QuickOpen => self.open_palette(true),
            Action::GlobalSearch => { self.sidebar_view = SideBarView::Search; self.sidebar_visible = true; self.overlay = Some(Overlay::GlobalSearch); }
            Action::Help => self.overlay = Some(Overlay::Help),
            Action::Refresh => {
                self.refresh_git();
                self.notify("Explorer refreshed", MessageKind::Info);
            }
            Action::TerminalNew => {
                let cwd = self.cwd();
                match self.terminal.spawn(&cwd) {
                    Ok(_) => { self.show_terminal(); self.focus = Focus::Terminal; }
                    Err(e) => self.notify(format!("Terminal failed: {}", e), MessageKind::Error),
                }
            }
            Action::TerminalClose => {
                self.terminal.close_active();
                if self.terminal.sessions.is_empty() { self.panel.visible = false; }
            }
            Action::TerminalNext => self.terminal.next(),
            Action::TerminalPrevious => self.terminal.previous(),
            Action::ExplorerNewFile => self.open_prompt(PromptKind::NewFile),
            Action::ExplorerNewFolder => self.open_prompt(PromptKind::NewFolder),
            Action::ExplorerRename => {
                if self.selected_path().is_some() { self.open_prompt(PromptKind::Rename); }
            }
            Action::ExplorerDelete => {
                if let Some(path) = self.selected_path() {
                    self.confirm = Some(ConfirmState::yes_no(
                        format!("Delete \"{}\"? This cannot be undone.", path.display()),
                        PendingAction::Delete(path),
                    ));
                    self.overlay = Some(Overlay::Confirm);
                }
            }
            Action::ExplorerCollapseAll => {
                if let Some(workspace) = &mut self.workspace {
                    collapse_all(&mut workspace.tree);
                }
            }
            Action::ExplorerRevealActive => {
                if let Some(path) = self.active_editor().and_then(|e| e.path()).map(|p| p.to_path_buf()) {
                    if let Some(workspace) = &mut self.workspace {
                        reveal_path(&mut workspace.tree, &path);
                    }
                    self.sidebar_view = SideBarView::Explorer;
                    self.sidebar_visible = true;
                }
            }
            Action::GitRefresh => { self.refresh_git(); self.notify("Git status refreshed", MessageKind::Info); }
            Action::GitCommit => {
                if self.workspace.as_ref().and_then(|w| w.git.root.clone()).is_some() {
                    self.open_prompt(PromptKind::GitCommit);
                } else {
                    self.notify("No git repository in this workspace", MessageKind::Warning);
                }
            }
            Action::ThemePicker => self.open_theme_picker(),
            Action::NextTheme => self.cycle_theme(1),
            Action::PreviousTheme => self.cycle_theme(-1),
            Action::SetTheme(ref name) => {
                if let Some(theme) = crate::themes::builtin(name) {
                    self.theme = theme;
                    self.config.theme = name.clone();
                    let _ = self.config.save();
                }
            }
            Action::RunActiveFile => self.run_active_file(),
            Action::ReloadFile => {
                if let Some(editor) = self.active_editor() {
                    let index = self.active;
                    if editor.path().is_some() {
                        self.confirm = Some(ConfirmState::yes_no(
                            format!("Revert \"{}\" to the version on disk?", editor.name()),
                            PendingAction::Reload(index),
                        ));
                        self.overlay = Some(Overlay::Confirm);
                    }
                }
            }
            Action::DeletePath(path) => {
                self.confirm = Some(ConfirmState::yes_no(
                    format!("Delete \"{}\"?", path.display()),
                    PendingAction::Delete(path),
                ));
                self.overlay = Some(Overlay::Confirm);
            }
        }
    }
}

pub fn refresh_browser_for(app: &mut App) {
    if let Some(browser) = &mut app.file_browser {
        refresh_browser(browser);
    }
}

fn resolve_input(base: &Path, input: &str) -> PathBuf {
    let p = PathBuf::from(input);
    if p.is_absolute() { p } else { base.join(p) }
}

fn shell_quote(s: &str) -> String {
    if !s.contains(|c: char| c.is_whitespace() || c == '\'' || c == '"' || c == '$') {
        return s.to_string();
    }
    if s.contains('\'') {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        format!("'{}'", s)
    }
}

fn find_cargo_manifest(path: &Path) -> Option<PathBuf> {
    let mut dir = path.parent();
    while let Some(d) = dir {
        let manifest = d.join("Cargo.toml");
        if manifest.is_file() { return Some(manifest); }
        dir = d.parent();
    }
    None
}

fn refresh_browser(browser: &mut FileBrowserState) {
    browser.entries.clear();
    let Ok(read) = std::fs::read_dir(&browser.dir) else { return; };
    let mut entries: Vec<_> = read.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| (!e.file_type().map(|t| t.is_dir()).unwrap_or(false), e.file_name().to_string_lossy().to_lowercase()));
    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') && browser.filter.is_empty() { continue; }
        if !browser.filter.is_empty() && !name.to_lowercase().contains(&browser.filter.to_lowercase()) { continue; }
        let path = entry.path();
        let is_dir = path.is_dir();
        if !is_dir && browser.mode == FileBrowserMode::OpenFolder { continue; }
        browser.entries.push((path, name, is_dir));
    }
    browser.selected = browser.selected.min(browser.entries.len().saturating_sub(1));
}

fn collapse_all(node: &mut crate::workspace::FileNode) {
    if node.depth > 0 { node.expanded = false; }
    for child in &mut node.children { collapse_all(child); }
}
