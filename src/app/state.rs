use super::action::Action;
use super::command;
use crate::editor::{CompletionEngine, CompletionState, Editor, Language};
use crate::services::fuzzy;
use crate::services::{Clipboard, Config};
use crate::terminal::TerminalManager;
use crate::themes::Theme;
use crate::workspace::search::{self, SearchResult};
use crate::workspace::{reveal_path, Watcher, Workspace, GitState};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Editor,
    Explorer,
    Terminal,
    Panel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    CommandPalette,
    QuickOpen,
    GotoLine,
    Find,
    Replace,
    GlobalSearch,
    Prompt,
    Confirm,
    Help,
    Menu,
    FileBrowser,
    ContextMenu,
    ThemePicker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SideBarView {
    Explorer,
    Search,
    SourceControl,
    RunDebug,
}

impl SideBarView {
    pub fn title(&self) -> &'static str {
        match self {
            SideBarView::Explorer => "EXPLORER",
            SideBarView::Search => "SEARCH",
            SideBarView::SourceControl => "SOURCE CONTROL",
            SideBarView::RunDebug => "RUN AND DEBUG",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PaletteItem {
    pub label: String,
    pub detail: String,
    pub action: Option<Action>,
    pub path: Option<PathBuf>,
    pub indices: Vec<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct PaletteState {
    pub query: String,
    pub selected: usize,
    pub items: Vec<PaletteItem>,
    pub scroll: usize,
}

#[derive(Debug, Clone, Default)]
pub struct FindState {
    pub query: String,
    pub replacement: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub matches: Vec<(usize, usize)>,
    pub current: usize,
    pub field: usize,
}

#[derive(Debug, Clone, Default)]
pub struct SearchState {
    pub query: String,
    pub replacement: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub show_replace: bool,
    pub results: Vec<SearchResult>,
    pub selected: usize,
    pub scroll: usize,
    pub field: usize,
}

#[derive(Debug, Clone)]
pub struct ThemePickerState {
    pub query: String,
    pub selected: usize,
    pub names: Vec<String>,
    /// Theme active before the picker opened, restored when cancelled.
    pub original: Theme,
    /// Index of the theme that was active when the picker opened.
    pub original_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WelcomeAction {
    NewFile,
    OpenFile,
    OpenFolder,
    Recent(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileBrowserMode {
    OpenFile,
    OpenFolder,
}

#[derive(Debug, Clone)]
pub struct FileBrowserState {
    pub dir: PathBuf,
    pub entries: Vec<(PathBuf, String, bool)>,
    pub selected: usize,
    pub filter: String,
    pub mode: FileBrowserMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextAction {
    Open,
    NewFile,
    NewFolder,
    Rename,
    Delete,
    CopyPath,
    Reveal,
}

impl ContextAction {
    pub fn label(&self) -> &'static str {
        match self {
            ContextAction::Open => "Open",
            ContextAction::NewFile => "New File",
            ContextAction::NewFolder => "New Folder",
            ContextAction::Rename => "Rename",
            ContextAction::Delete => "Delete",
            ContextAction::CopyPath => "Copy Path",
            ContextAction::Reveal => "Reveal in Explorer",
        }
    }
    pub fn hint(&self) -> &'static str {
        match self {
            ContextAction::Open => "Enter",
            ContextAction::Rename => "F2",
            ContextAction::Delete => "Del",
            ContextAction::NewFile => "",
            ContextAction::NewFolder => "",
            ContextAction::CopyPath => "",
            ContextAction::Reveal => "",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContextMenuState {
    pub path: PathBuf,
    pub is_dir: bool,
    pub actions: Vec<ContextAction>,
    pub selected: usize,
    pub x: u16,
    pub y: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    NewFile,
    NewFolder,
    Rename,
    SaveAs,
    GotoLine,
    GitCommit,
}

#[derive(Debug, Clone)]
pub struct PromptState {
    pub title: String,
    pub value: String,
    pub cursor: usize,
    pub kind: PromptKind,
    pub target: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub enum PendingAction {
    Delete(PathBuf),
    CloseTab(usize),
    Quit,
    Reload(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmStyle {
    YesNo,
    Exit,
}

#[derive(Debug, Clone)]
pub struct ConfirmState {
    pub message: String,
    pub action: PendingAction,
    pub style: ConfirmStyle,
    pub selected: usize,
}

impl ConfirmState {
    pub fn yes_no(message: String, action: PendingAction) -> Self {
        Self { message, action, style: ConfirmStyle::YesNo, selected: 0 }
    }
    pub fn exit() -> Self {
        Self {
            message: "There are unsaved changes in your workspace.".into(),
            action: PendingAction::Quit,
            style: ConfirmStyle::Exit,
            selected: 0, // "Save All and Exit" is the safe default
        }
    }
    pub fn option_count(&self) -> usize {
        match self.style { ConfirmStyle::YesNo => 2, ConfirmStyle::Exit => 3 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelTab {
    Problems,
    Output,
    DebugConsole,
    Terminal,
}

impl PanelTab {
    pub fn label(&self) -> &'static str {
        match self {
            PanelTab::Problems => "PROBLEMS",
            PanelTab::Output => "OUTPUT",
            PanelTab::DebugConsole => "DEBUG CONSOLE",
            PanelTab::Terminal => "TERMINAL",
        }
    }
    pub fn all() -> [PanelTab; 4] {
        [PanelTab::Problems, PanelTab::Output, PanelTab::DebugConsole, PanelTab::Terminal]
    }
}

#[derive(Debug, Clone)]
pub struct PanelState {
    pub tab: PanelTab,
    pub visible: bool,
    pub scroll: usize,
}

#[derive(Debug, Clone, Default)]
pub struct MenuState {
    pub open: Option<usize>,
    pub selected: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub path: PathBuf,
    pub line: usize,
    pub col: usize,
    pub message: String,
    pub severity: Severity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub text: String,
    pub kind: MessageKind,
    pub at: Instant,
}

/// Snapshot of transient UI state taken when an exit is requested, so that
/// Cancel can restore the user's exact previous context.
#[derive(Debug, Clone)]
pub struct UiSnapshot {
    pub focus: Focus,
    pub overlay: Option<Overlay>,
    pub active: usize,
    pub cursor: usize,
    pub anchor: Option<usize>,
    pub palette_query: String,
    pub palette_selected: usize,
    pub find_query: String,
    pub search_query: String,
    pub prompt: Option<PromptState>,
    pub context_menu: Option<ContextMenuState>,
    pub menu: MenuState,
    pub sidebar_view: SideBarView,
    pub sidebar_visible: bool,
}

impl UiSnapshot {
    pub fn capture(app: &App) -> Self {
        let (cursor, anchor) = app.active_editor()
            .map(|e| (e.view.cursor, e.view.anchor))
            .unwrap_or((0, None));
        Self {
            focus: app.focus,
            overlay: app.overlay,
            active: app.active,
            cursor,
            anchor,
            palette_query: app.palette.query.clone(),
            palette_selected: app.palette.selected,
            find_query: app.find.query.clone(),
            search_query: app.search.query.clone(),
            prompt: app.prompt.clone(),
            context_menu: app.context_menu.clone(),
            menu: app.menu.clone(),
            sidebar_view: app.sidebar_view,
            sidebar_visible: app.sidebar_visible,
        }
    }

    pub fn restore(&self, app: &mut App) {
        app.focus = self.focus;
        app.overlay = self.overlay;
        app.active = self.active.min(app.editors.len().saturating_sub(1));
        if let Some(editor) = app.active_editor_mut() {
            editor.view.cursor = self.cursor.min(editor.buffer.len_chars());
            editor.view.anchor = self.anchor.map(|a| a.min(editor.buffer.len_chars()));
        }
        app.palette.query = self.palette_query.clone();
        app.palette.selected = self.palette_selected;
        app.find.query = self.find_query.clone();
        app.search.query = self.search_query.clone();
        app.prompt = self.prompt.clone();
        app.context_menu = self.context_menu.clone();
        app.menu = self.menu.clone();
        app.sidebar_view = self.sidebar_view;
        app.sidebar_visible = self.sidebar_visible;
    }
}

/// The complete application state.
pub struct App {
    pub config: Config,
    pub theme: Theme,
    pub editors: Vec<Editor>,
    pub active: usize,
    pub next_id: usize,
    pub workspace: Option<Workspace>,
    pub focus: Focus,
    pub overlay: Option<Overlay>,
    pub sidebar_view: SideBarView,
    pub sidebar_visible: bool,
    pub panel: PanelState,
    pub explorer_selected: usize,
    pub explorer_scroll: usize,
    pub palette: PaletteState,
    pub find: FindState,
    pub search: SearchState,
    pub prompt: Option<PromptState>,
    pub confirm: Option<ConfirmState>,
    pub file_browser: Option<FileBrowserState>,
    pub context_menu: Option<ContextMenuState>,
    /// Base directory for a create-file/folder prompt opened from a context menu.
    pub cwd_for_prompt: Option<PathBuf>,
    pub recent_projects: Vec<PathBuf>,
    pub welcome_selected: usize,
    pub diagnostics: Vec<Diagnostic>,
    pub output: Vec<String>,
    pub messages: Vec<Message>,
    pub terminal: TerminalManager,
    pub clipboard: Clipboard,
    pub menu: MenuState,
    pub quit: bool,
    pub terminal_size: (u16, u16),
    pub last_autosave: Instant,
    pub watcher: Watcher,
    pub show_welcome: bool,
    pub chord_k: bool,
    pub last_click: Option<(u16, u16, Instant)>,
    /// True once an exit has been requested; prevents duplicate exit requests.
    pub exit_requested: bool,
    pub exit_snapshot: Option<UiSnapshot>,
    pub theme_picker: Option<ThemePickerState>,
    pub completion: CompletionState,
    pub completion_engine: CompletionEngine,
}

impl App {
    pub fn new(config: Config, theme: Theme, root: Option<PathBuf>, cols: usize, rows: usize) -> Self {
        let workspace = root.and_then(|p| Workspace::open(&p, &config).ok());
        let mut app = Self {
            config,
            theme,
            editors: Vec::new(),
            active: 0,
            next_id: 1,
            workspace,
            focus: Focus::Editor,
            overlay: None,
            sidebar_view: SideBarView::Explorer,
            sidebar_visible: true,
            panel: PanelState { tab: PanelTab::Terminal, visible: false, scroll: 0 },
            explorer_selected: 0,
            explorer_scroll: 0,
            palette: PaletteState::default(),
            find: FindState::default(),
            search: SearchState::default(),
            prompt: None,
            confirm: None,
            file_browser: None,
            context_menu: None,
            cwd_for_prompt: None,
            recent_projects: crate::platform::load_recent_projects(8),
            welcome_selected: 0,
            diagnostics: Vec::new(),
            output: vec![
                "AMO VS - terminal code editor".into(),
                format!("Version {}", env!("CARGO_PKG_VERSION")),
                "Ready.".into(),
            ],
            messages: Vec::new(),
            terminal: TerminalManager::new(cols, rows.saturating_sub(8).max(4), None, 1000),
            clipboard: Clipboard::new(),
            menu: MenuState::default(),
            quit: false,
            terminal_size: (cols as u16, rows as u16),
            last_autosave: Instant::now(),
            watcher: Watcher::new(),
            show_welcome: true,
            chord_k: false,
            last_click: None,
            exit_requested: false,
            exit_snapshot: None,
            theme_picker: None,
            completion: CompletionState::default(),
            completion_engine: CompletionEngine::new(),
        };
        app.configure_terminal();
        app.open_welcome();
        app
    }

    fn configure_terminal(&mut self) {
        if let Some(shell) = self.config.shell.clone() {
            let mut cfg = crate::platform::default_shell();
            cfg.program = shell;
            self.terminal = TerminalManager::new(
                self.terminal.width,
                self.terminal.height,
                Some(cfg),
                self.config.terminal_history_size,
            );
        } else {
            self.terminal = TerminalManager::new(
                self.terminal.width,
                self.terminal.height,
                None,
                self.config.terminal_history_size,
            );
        }
    }

    pub fn open_welcome(&mut self) {
        let text = concat!(
            "Welcome to AMO VS\n",
            "=================\n\n",
            "A VS Code-inspired editor for your terminal.\n\n",
            "Quick start\n",
            "  Ctrl+P          Go to File\n",
            "  Ctrl+Shift+P    Command Palette\n",
            "  Ctrl+O          Open File\n",
            "  Ctrl+K Ctrl+O   Open Folder\n",
            "  Ctrl+B          Toggle Side Bar\n",
            "  Ctrl+`          Toggle Terminal\n",
            "  Ctrl+F          Find\n",
            "  Ctrl+Shift+F    Find in Files\n",
            "  F1              Command Palette\n\n",
            "Edit this file, open a folder, or press Ctrl+O to begin.\n",
        );
        let id = self.next_id;
        self.next_id += 1;
        let mut editor = Editor::scratch(id, text, Language::Markdown);
        editor.is_welcome = true;
        self.editors.push(editor);
        self.active = self.editors.len() - 1;
        self.show_welcome = true;
    }

    pub fn welcome_actions(&self) -> Vec<WelcomeAction> {
        let mut actions = vec![WelcomeAction::NewFile, WelcomeAction::OpenFile, WelcomeAction::OpenFolder];
        for i in 0..self.recent_projects.len().min(5) {
            actions.push(WelcomeAction::Recent(i));
        }
        actions
    }

    pub fn activate_welcome(&mut self, index: usize) {
        let Some(action) = self.welcome_actions().get(index).copied() else { return; };
        self.welcome_selected = index;
        match action {
            WelcomeAction::NewFile => self.execute(Action::NewFile),
            WelcomeAction::OpenFile => self.execute(Action::OpenFile),
            WelcomeAction::OpenFolder => self.execute(Action::OpenFolder),
            WelcomeAction::Recent(i) => {
                if let Some(path) = self.recent_projects.get(i).cloned() {
                    if let Err(e) = self.open_workspace(path) {
                        self.notify(format!("Open folder failed: {}", e), MessageKind::Error);
                    }
                }
            }
        }
    }

    // ---- Editor access ---------------------------------------------------

    pub fn active_editor(&self) -> Option<&Editor> { self.editors.get(self.active) }

    pub fn active_editor_mut(&mut self) -> Option<&mut Editor> { self.editors.get_mut(self.active) }

    pub fn open_path(&mut self, path: &Path) -> io::Result<()> {
        let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        if let Some(index) = self.editors.iter().position(|e| e.path() == Some(path.as_path())) {
            self.active = index;
            self.focus = Focus::Editor;
            self.show_welcome = false;
            return Ok(());
        }
        let id = self.next_id;
        self.next_id += 1;
        let editor = Editor::open(id, &path)?;
        if let Some(parent) = path.parent() {
            crate::platform::remember_project(parent, 10);
            self.recent_projects = crate::platform::load_recent_projects(8);
        }
        self.editors.push(editor);
        self.active = self.editors.len() - 1;
        self.focus = Focus::Editor;
        self.show_welcome = false;
        if let Some(workspace) = &mut self.workspace {
            reveal_path(&mut workspace.tree, &path);
            workspace.select(path);
        }
        self.refresh_diagnostics();
        Ok(())
    }

    pub fn new_untitled(&mut self) {
        let id = self.next_id;
        self.next_id += 1;
        self.editors.push(Editor::scratch(id, "", Language::PlainText));
        self.active = self.editors.len() - 1;
        self.focus = Focus::Editor;
        self.show_welcome = false;
    }

    pub fn close_tab(&mut self, index: usize) {
        if index >= self.editors.len() { return; }
        if self.editors[index].dirty() {
            self.confirm = Some(ConfirmState::yes_no(
                format!("\"{}\" has unsaved changes. Save before closing?", self.editors[index].name()),
                PendingAction::CloseTab(index),
            ));
            self.overlay = Some(Overlay::Confirm);
            return;
        }
        self.editors.remove(index);
        if index < self.active { self.active = self.active.saturating_sub(1); }
        self.active = self.active.min(self.editors.len().saturating_sub(1));
        if self.editors.is_empty() {
            self.new_untitled();
        }
        self.refresh_diagnostics();
    }

    pub fn save(&mut self) {
        let Some(editor) = self.active_editor_mut() else { return; };
        if editor.path().is_none() {
            self.open_prompt(PromptKind::SaveAs);
            return;
        }
        match editor.save() {
            Ok(()) => {
                self.notify("File saved", MessageKind::Info);
                self.refresh_git();
            }
            Err(e) => self.notify(format!("Save failed: {}", e), MessageKind::Error),
        }
    }

    pub fn save_all(&mut self) {
        match self.save_all_result() {
            Ok(count) => {
                self.notify(format!("Saved {} file(s)", count), MessageKind::Info);
                self.refresh_git();
            }
            Err(error) => self.notify(format!("Save failed: {}", error), MessageKind::Error),
        }
    }

    pub fn notify(&mut self, text: impl Into<String>, kind: MessageKind) {
        self.messages.push(Message { text: text.into(), kind, at: Instant::now() });
        if self.messages.len() > 20 { self.messages.remove(0); }
    }

    pub fn log(&mut self, text: impl Into<String>) {
        self.output.push(text.into());
        if self.output.len() > 2000 { self.output.remove(0); }
    }

    pub fn refresh_git(&mut self) {
        if let Some(workspace) = &mut self.workspace {
            if self.config.git_enabled {
                workspace.refresh(&self.config);
            }
        }
    }

    pub fn refresh_diagnostics(&mut self) {
        self.diagnostics.clear();
        for editor in &mut self.editors {
            let Some(path) = editor.path().map(|p| p.to_path_buf()) else { continue; };
            let _ = editor.highlighter.spans(&editor.buffer, editor.buffer.revision());
            for d in editor.highlighter.diagnostics() {
                self.diagnostics.push(Diagnostic {
                    path: path.clone(),
                    line: d.line,
                    col: d.col,
                    message: d.message,
                    severity: match d.severity {
                        crate::editor::DiagnosticSeverity::Error => Severity::Error,
                        crate::editor::DiagnosticSeverity::Warning => Severity::Warning,
                    },
                });
            }
        }
    }

    // ---- Overlays --------------------------------------------------------

    pub fn open_prompt(&mut self, kind: PromptKind) {
        let (title, value, target) = match kind {
            PromptKind::NewFile => ("New File", String::new(), None),
            PromptKind::NewFolder => ("New Folder", String::new(), None),
            PromptKind::Rename => {
                let target = self.selected_path();
                let value = target.as_ref().and_then(|p| p.file_name())
                    .map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                ("Rename", value, target)
            }
            PromptKind::SaveAs => {
                let value = self.active_editor()
                    .map(|e| e.name())
                    .unwrap_or_else(|| "untitled.txt".into());
                ("Save As", value, None)
            }
            PromptKind::GotoLine => ("Go to Line", String::new(), None),
            PromptKind::GitCommit => ("Git Commit Message", String::new(), None),
        };
        let cursor = value.chars().count();
        self.prompt = Some(PromptState { title: title.into(), value, cursor, kind, target });
        self.overlay = Some(Overlay::Prompt);
    }

    pub fn open_palette(&mut self, quick_open: bool) {
        self.palette.query = if quick_open { String::new() } else { ">".into() };
        self.palette.selected = 0;
        self.palette.scroll = 0;
        self.overlay = Some(if quick_open { Overlay::QuickOpen } else { Overlay::CommandPalette });
        self.update_palette();
    }

    pub fn update_palette(&mut self) {
        let is_command = self.palette.query.trim_start().starts_with('>');
        let query = self.palette.query.trim().trim_start_matches('>').trim().to_string();
        let mut items = Vec::new();
        if !is_command {
            let mut paths: Vec<PathBuf> = Vec::new();
            if let Some(workspace) = &self.workspace {
                collect_files(&workspace.root, &self.config.exclude, 0, &mut paths);
            }
            for editor in &self.editors {
                if let Some(path) = editor.path() { paths.push(path.to_path_buf()); }
            }
            paths.sort();
            paths.dedup();
            let strings: Vec<String> = paths.iter().map(|p| p.to_string_lossy().to_string()).collect();
            for score_result in fuzzy::fuzzy_filter(&query, &strings, 200) {
                let (idx, _score, indices) = score_result;
                let path = paths[idx].clone();
                let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                items.push(PaletteItem {
                    label: name,
                    detail: path.to_string_lossy().to_string(),
                    action: None,
                    path: Some(path),
                    indices,
                });
            }
        } else {
            let commands = command::palette_commands();
            let labels: Vec<String> = commands.iter().map(|a| a.label()).collect();
            for (idx, _score, indices) in fuzzy::fuzzy_filter(&query, &labels, 200) {
                let action = commands[idx].clone();
                items.push(PaletteItem {
                    label: action.label(),
                    detail: format!("{}  ", action.category()),
                    action: Some(action),
                    path: None,
                    indices,
                });
            }
        }
        self.palette.items = items;
        if self.palette.selected >= self.palette.items.len() {
            self.palette.selected = self.palette.items.len().saturating_sub(1);
        }
    }

    pub fn perform_search(&mut self) {
        if self.search.query.is_empty() {
            self.search.results.clear();
            return;
        }
        let Some(workspace) = &self.workspace else { return; };
        self.search.results = search::search(
            &workspace.root,
            &self.search.query,
            &self.config,
            self.search.case_sensitive,
            self.search.whole_word,
            500,
        );
        self.search.selected = 0;
        self.search.scroll = 0;
    }

    pub fn find_in_editor(&mut self) {
        let Some(editor) = self.active_editor() else { return; };
        self.find.matches = editor.buffer.find_all(
            &self.find.query,
            self.find.case_sensitive,
            self.find.whole_word,
        );
        if self.find.current >= self.find.matches.len() { self.find.current = 0; }
    }

    pub fn goto_match(&mut self, index: usize) {
        let found = self.find.matches.get(index).copied();
        if let Some(editor) = self.active_editor_mut() {
            if let Some((start, end)) = found {
                editor.view.cursor = end;
                editor.view.anchor = Some(start);
                editor.view.desired_col = end - editor.buffer.line_to_char(editor.buffer.char_to_line(end));
                self.find.current = index;
            }
        }
    }

    // ---- Helpers ---------------------------------------------------------

    pub fn selected_path(&self) -> Option<PathBuf> {
        let workspace = self.workspace.as_ref()?;
        workspace.flatten().get(self.explorer_selected).map(|e| e.path.clone())
    }

    pub fn cwd(&self) -> PathBuf {
        self.workspace.as_ref().map(|w| w.root.clone())
            .or_else(|| self.active_editor().and_then(|e| e.path()).and_then(|p| p.parent().map(|p| p.to_path_buf())))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }

    pub fn refresh_timers(&mut self, now: Instant) {
        if self.config.autosave && now.duration_since(self.last_autosave) >= Duration::from_millis(self.config.autosave_delay_ms) {
            self.last_autosave = now;
            let mut saved = 0;
            for editor in &mut self.editors {
                if editor.dirty() && editor.path().is_some() && editor.save().is_ok() { saved += 1; }
            }
            if saved > 0 { self.refresh_git(); }
        }
        let cutoff = Duration::from_secs(6);
        self.messages.retain(|m| now.duration_since(m.at) < cutoff);
    }

    pub fn check_files(&mut self) {
        let files: Vec<PathBuf> = self.editors.iter().filter_map(|e| e.path().map(|p| p.to_path_buf())).collect();
        let root = self.cwd();
        if self.watcher.tick(&root, &files, Duration::from_secs(2)) {
            let changed: Vec<usize> = self.editors.iter().enumerate()
                .filter(|(_, e)| {
                    e.path().map(|p| {
                        std::fs::metadata(p).and_then(|m| m.modified()).ok() != e.mtime
                    }).unwrap_or(false)
                })
                .map(|(i, _)| i)
                .collect();
            for index in changed {
                if !self.editors[index].dirty() {
                    let _ = self.editors[index].reload();
                    self.notify("File reloaded from disk", MessageKind::Info);
                } else {
                    self.editors[index].external_change = true;
                }
            }
            self.refresh_git();
        }
    }

    pub fn active_diagnostic_count(&self) -> (usize, usize) {
        let errors = self.diagnostics.iter().filter(|d| d.severity == Severity::Error).count();
        let warnings = self.diagnostics.iter().filter(|d| d.severity == Severity::Warning).count();
        (errors, warnings)
    }

    pub fn git_changes(&self) -> usize {
        self.workspace.as_ref().map(|w| w.git.changes.values()
            .filter(|s| **s != GitState::Clean && **s != GitState::Ignored).count()).unwrap_or(0)
    }
}

fn collect_files(root: &Path, excludes: &[String], depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 16 { return; }
    let Ok(read) = std::fs::read_dir(root) else { return; };
    for entry in read.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().to_string();
        if excludes.iter().any(|p| {
            if let Some(s) = p.strip_prefix('*') { name.ends_with(s) } else { name == *p }
        }) { continue; }
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, excludes, depth + 1, out);
        } else if path.is_file() {
            out.push(path);
        }
    }
}
