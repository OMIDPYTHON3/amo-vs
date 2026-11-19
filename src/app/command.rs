use super::action::Action;

#[derive(Debug, Clone)]
pub struct MenuEntry {
    pub label: &'static str,
    pub hint: &'static str,
    pub action: Option<Action>,
    pub separator: bool,
}

impl MenuEntry {
    fn item(label: &'static str, hint: &'static str, action: Action) -> Self {
        Self { label, hint, action: Some(action), separator: false }
    }
    fn sep() -> Self {
        Self { label: "", hint: "", action: None, separator: true }
    }
}

pub const MENU_NAMES: [&str; 8] = ["File", "Edit", "Selection", "View", "Go", "Run", "Terminal", "Help"];

pub fn menu_entries(index: usize) -> Vec<MenuEntry> {
    match index {
        0 => vec![
            MenuEntry::item("New Text File", "Ctrl+N", Action::NewFile),
            MenuEntry::item("Open File...", "Ctrl+O", Action::OpenFile),
            MenuEntry::item("Open Folder...", "Ctrl+K Ctrl+O", Action::OpenFolder),
            MenuEntry::sep(),
            MenuEntry::item("Save", "Ctrl+S", Action::Save),
            MenuEntry::item("Save As...", "Ctrl+Shift+S", Action::SaveAs),
            MenuEntry::item("Save All", "Ctrl+K S", Action::SaveAll),
            MenuEntry::sep(),
            MenuEntry::item("Revert File", "", Action::ReloadFile),
            MenuEntry::sep(),
            MenuEntry::item("Exit", "Ctrl+Esc", Action::Quit),
        ],
        1 => vec![
            MenuEntry::item("Undo", "Ctrl+Z", Action::Undo),
            MenuEntry::item("Redo", "Ctrl+Y", Action::Redo),
            MenuEntry::sep(),
            MenuEntry::item("Cut", "Ctrl+X", Action::Cut),
            MenuEntry::item("Copy", "Ctrl+C", Action::Copy),
            MenuEntry::item("Paste", "Ctrl+V", Action::Paste),
            MenuEntry::sep(),
            MenuEntry::item("Find", "Ctrl+F", Action::Find),
            MenuEntry::item("Replace", "Ctrl+H", Action::Replace),
            MenuEntry::item("Find in Files", "Ctrl+Shift+F", Action::GlobalSearch),
            MenuEntry::sep(),
            MenuEntry::item("Toggle Line Comment", "Ctrl+/", Action::ToggleComment),
        ],
        2 => vec![
            MenuEntry::item("Select All", "Ctrl+A", Action::SelectAll),
            MenuEntry::sep(),
            MenuEntry::item("Move Cursor Left", "←", Action::CursorLeft),
            MenuEntry::item("Move Cursor Right", "→", Action::CursorRight),
            MenuEntry::item("Move Word Left", "Ctrl+←", Action::WordLeft),
            MenuEntry::item("Move Word Right", "Ctrl+→", Action::WordRight),
            MenuEntry::sep(),
            MenuEntry::item("Top of File", "Ctrl+Home", Action::DocStart),
            MenuEntry::item("End of File", "Ctrl+End", Action::DocEnd),
        ],
        3 => vec![
            MenuEntry::item("Command Palette", "Ctrl+Shift+P", Action::CommandPalette),
            MenuEntry::item("Go to File...", "Ctrl+P", Action::QuickOpen),
            MenuEntry::sep(),
            MenuEntry::item("Toggle Primary Side Bar", "Ctrl+B", Action::ToggleSidebar),
            MenuEntry::item("Toggle Panel", "Ctrl+J", Action::TogglePanel),
            MenuEntry::item("Toggle Status Bar", "", Action::ToggleStatusBar),
            MenuEntry::item("Toggle Word Wrap", "Alt+Z", Action::ToggleWordWrap),
            MenuEntry::item("Toggle Line Numbers", "", Action::ToggleLineNumbers),
            MenuEntry::item("Color Theme...", "", Action::ThemePicker),
            MenuEntry::sep(),
            MenuEntry::item("Next Editor", "Ctrl+Tab", Action::NextTab),
            MenuEntry::item("Previous Editor", "Ctrl+Shift+Tab", Action::PreviousTab),
            MenuEntry::item("Close Editor", "Ctrl+W", Action::CloseTab),
        ],
        4 => vec![
            MenuEntry::item("Go to File...", "Ctrl+P", Action::QuickOpen),
            MenuEntry::item("Go to Line/Column...", "Ctrl+G", Action::GotoLine),
            MenuEntry::sep(),
            MenuEntry::item("Go to Next Editor", "Ctrl+Tab", Action::NextTab),
            MenuEntry::item("Go to Previous Editor", "Ctrl+Shift+Tab", Action::PreviousTab),
            MenuEntry::item("Reveal Active File in Explorer", "", Action::ExplorerRevealActive),
        ],
        5 => vec![
            MenuEntry::item("Run Active File", "Ctrl+F5", Action::RunActiveFile),
            MenuEntry::sep(),
            MenuEntry::item("Toggle Terminal", "Ctrl+`", Action::ToggleTerminal),
        ],
        6 => vec![
            MenuEntry::item("New Terminal", "Ctrl+Shift+`", Action::TerminalNew),
            MenuEntry::item("Toggle Terminal", "Ctrl+`", Action::ToggleTerminal),
            MenuEntry::sep(),
            MenuEntry::item("Next Terminal", "", Action::TerminalNext),
            MenuEntry::item("Previous Terminal", "", Action::TerminalPrevious),
            MenuEntry::item("Kill Terminal", "", Action::TerminalClose),
        ],
        _ => vec![
            MenuEntry::item("Welcome", "", Action::Help),
            MenuEntry::item("Keyboard Shortcuts", "", Action::Help),
        ],
    }
}

/// Commands shown in the Command Palette (Ctrl+Shift+P).
pub fn palette_commands() -> Vec<Action> {
    vec![
        Action::NewFile,
        Action::OpenFile,
        Action::OpenFolder,
        Action::Save,
        Action::SaveAs,
        Action::SaveAll,
        Action::CloseTab,
        Action::CloseOtherTabs,
        Action::ReloadFile,
        Action::Undo,
        Action::Redo,
        Action::Cut,
        Action::Copy,
        Action::Paste,
        Action::SelectAll,
        Action::Find,
        Action::Replace,
        Action::FindNext,
        Action::FindPrevious,
        Action::ToggleComment,
        Action::Indent,
        Action::Outdent,
        Action::DuplicateLine,
        Action::DeleteLine,
        Action::GotoLine,
        Action::ToggleSidebar,
        Action::TogglePanel,
        Action::ToggleTerminal,
        Action::ToggleStatusBar,
        Action::ToggleWordWrap,
        Action::ToggleLineNumbers,
        Action::FocusExplorer,
        Action::FocusEditor,
        Action::FocusTerminal,
        Action::NextTab,
        Action::PreviousTab,
        Action::NextPanelTab,
        Action::PreviousPanelTab,
        Action::CommandPalette,
        Action::QuickOpen,
        Action::GlobalSearch,
        Action::Help,
        Action::Refresh,
        Action::TerminalNew,
        Action::TerminalClose,
        Action::TerminalNext,
        Action::TerminalPrevious,
        Action::ExplorerNewFile,
        Action::ExplorerNewFolder,
        Action::ExplorerRename,
        Action::ExplorerDelete,
        Action::ExplorerCollapseAll,
        Action::ExplorerRevealActive,
        Action::GitRefresh,
        Action::GitCommit,
        Action::RunActiveFile,
        Action::ThemePicker,
        Action::NextTheme,
        Action::PreviousTheme,
        Action::Quit,
    ]
}
