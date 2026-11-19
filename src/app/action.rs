use serde::{Deserialize, Serialize};

/// Every user-triggerable operation in the editor. Commands, keybindings and
/// mouse actions all funnel through `App::execute`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    // File
    NewFile,
    OpenFile,
    OpenFolder,
    Save,
    SaveAs,
    SaveAll,
    CloseTab,
    CloseOtherTabs,
    Quit,

    // Edit
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Find,
    Replace,
    FindNext,
    FindPrevious,
    ToggleComment,
    Indent,
    Outdent,
    DuplicateLine,
    DeleteLine,
    GotoLine,

    // Cursor
    CursorLeft,
    CursorRight,
    CursorUp,
    CursorDown,
    WordLeft,
    WordRight,
    CursorHome,
    CursorEnd,
    DocStart,
    DocEnd,
    PageUp,
    PageDown,

    // View / navigation
    ToggleSidebar,
    TogglePanel,
    ToggleTerminal,
    ToggleStatusBar,
    ToggleWordWrap,
    ToggleLineNumbers,
    FocusExplorer,
    FocusEditor,
    FocusTerminal,
    NextTab,
    PreviousTab,
    NextPanelTab,
    PreviousPanelTab,
    CommandPalette,
    QuickOpen,
    GlobalSearch,
    Help,
    Refresh,

    // Terminal
    TerminalNew,
    TerminalClose,
    TerminalNext,
    TerminalPrevious,

    // Explorer
    ExplorerNewFile,
    ExplorerNewFolder,
    ExplorerRename,
    ExplorerDelete,
    ExplorerCollapseAll,
    ExplorerRevealActive,

    // Git
    GitRefresh,
    GitCommit,

    // Appearance
    ThemePicker,
    NextTheme,
    PreviousTheme,
    SetTheme(String),

    // Run
    RunActiveFile,

    // Application lifecycle (centralized exit flow)
    SaveAllAndExit,
    ExitWithoutSaving,
    CancelExit,

    // File operations (used by confirmations)
    ReloadFile,
    DeletePath(std::path::PathBuf),
}

impl Action {
    pub fn label(&self) -> String {
        match self {
            Action::NewFile => "File: New Text File".into(),
            Action::OpenFile => "File: Open File...".into(),
            Action::OpenFolder => "File: Open Folder...".into(),
            Action::Save => "File: Save".into(),
            Action::SaveAs => "File: Save As...".into(),
            Action::SaveAll => "File: Save All".into(),
            Action::CloseTab => "View: Close Editor".into(),
            Action::CloseOtherTabs => "View: Close Other Editors".into(),
            Action::Quit => "File: Exit".into(),
            Action::Undo => "Edit: Undo".into(),
            Action::Redo => "Edit: Redo".into(),
            Action::Cut => "Edit: Cut".into(),
            Action::Copy => "Edit: Copy".into(),
            Action::Paste => "Edit: Paste".into(),
            Action::SelectAll => "Edit: Select All".into(),
            Action::Find => "Edit: Find".into(),
            Action::Replace => "Edit: Replace".into(),
            Action::FindNext => "Edit: Find Next".into(),
            Action::FindPrevious => "Edit: Find Previous".into(),
            Action::ToggleComment => "Edit: Toggle Line Comment".into(),
            Action::Indent => "Edit: Indent Line".into(),
            Action::Outdent => "Edit: Outdent Line".into(),
            Action::DuplicateLine => "Edit: Duplicate Line".into(),
            Action::DeleteLine => "Edit: Delete Line".into(),
            Action::GotoLine => "Go to Line/Column...".into(),
            Action::CursorLeft => "Cursor: Move Left".into(),
            Action::CursorRight => "Cursor: Move Right".into(),
            Action::CursorUp => "Cursor: Move Up".into(),
            Action::CursorDown => "Cursor: Move Down".into(),
            Action::WordLeft => "Cursor: Word Left".into(),
            Action::WordRight => "Cursor: Word Right".into(),
            Action::CursorHome => "Cursor: Home".into(),
            Action::CursorEnd => "Cursor: End".into(),
            Action::DocStart => "Cursor: Top of File".into(),
            Action::DocEnd => "Cursor: End of File".into(),
            Action::PageUp => "Cursor: Page Up".into(),
            Action::PageDown => "Cursor: Page Down".into(),
            Action::ToggleSidebar => "View: Toggle Primary Side Bar".into(),
            Action::TogglePanel => "View: Toggle Panel".into(),
            Action::ToggleTerminal => "Terminal: Toggle Terminal".into(),
            Action::ToggleStatusBar => "View: Toggle Status Bar".into(),
            Action::ToggleWordWrap => "View: Toggle Word Wrap".into(),
            Action::ToggleLineNumbers => "View: Toggle Line Numbers".into(),
            Action::FocusExplorer => "View: Focus Explorer".into(),
            Action::FocusEditor => "View: Focus Editor".into(),
            Action::FocusTerminal => "Terminal: Focus Terminal".into(),
            Action::NextTab => "View: Next Editor".into(),
            Action::PreviousTab => "View: Previous Editor".into(),
            Action::NextPanelTab => "View: Next Panel Tab".into(),
            Action::PreviousPanelTab => "View: Previous Panel Tab".into(),
            Action::CommandPalette => "Show All Commands".into(),
            Action::QuickOpen => "Go to File...".into(),
            Action::GlobalSearch => "Search: Find in Files".into(),
            Action::Help => "Help: Welcome".into(),
            Action::Refresh => "View: Refresh Explorer".into(),
            Action::TerminalNew => "Terminal: New Terminal".into(),
            Action::TerminalClose => "Terminal: Kill Terminal".into(),
            Action::TerminalNext => "Terminal: Next Terminal".into(),
            Action::TerminalPrevious => "Terminal: Previous Terminal".into(),
            Action::ExplorerNewFile => "Explorer: New File".into(),
            Action::ExplorerNewFolder => "Explorer: New Folder".into(),
            Action::ExplorerRename => "Explorer: Rename".into(),
            Action::ExplorerDelete => "Explorer: Delete".into(),
            Action::ExplorerCollapseAll => "Explorer: Collapse Folders".into(),
            Action::ExplorerRevealActive => "Explorer: Reveal Active File".into(),
            Action::GitRefresh => "Git: Refresh".into(),
            Action::GitCommit => "Git: Commit Staged Changes".into(),
            Action::ThemePicker => "Preferences: Color Theme".into(),
            Action::NextTheme => "Preferences: Next Color Theme".into(),
            Action::PreviousTheme => "Preferences: Previous Color Theme".into(),
            Action::SetTheme(ref name) => format!("Preferences: Color Theme: {}", name),
            Action::RunActiveFile => "Run: Run Active File".into(),
            Action::SaveAllAndExit => "File: Save All and Exit".into(),
            Action::ExitWithoutSaving => "File: Exit Without Saving".into(),
            Action::CancelExit => "Cancel".into(),
            Action::ReloadFile => "File: Revert File".into(),
            Action::DeletePath(_) => "Delete".into(),
        }
    }

    pub fn category(&self) -> &'static str {
        match self {
            Action::NewFile | Action::OpenFile | Action::OpenFolder | Action::Save | Action::SaveAs
            | Action::SaveAll | Action::CloseTab | Action::CloseOtherTabs | Action::Quit
            | Action::ReloadFile | Action::SaveAllAndExit | Action::ExitWithoutSaving
            | Action::CancelExit => "File",
            Action::TerminalNew | Action::TerminalClose | Action::TerminalNext | Action::TerminalPrevious
            | Action::ToggleTerminal | Action::FocusTerminal => "Terminal",
            Action::ExplorerNewFile | Action::ExplorerNewFolder | Action::ExplorerRename
            | Action::ExplorerDelete | Action::ExplorerCollapseAll | Action::ExplorerRevealActive => "Explorer",
            Action::GitRefresh | Action::GitCommit => "Git",
            Action::ThemePicker | Action::NextTheme | Action::PreviousTheme | Action::SetTheme(_) => "Preferences",
            Action::RunActiveFile => "Run",
            Action::ToggleSidebar | Action::TogglePanel | Action::ToggleStatusBar | Action::FocusExplorer
            | Action::FocusEditor | Action::NextTab | Action::PreviousTab | Action::NextPanelTab
            | Action::PreviousPanelTab | Action::Refresh | Action::Help | Action::ToggleWordWrap
            | Action::ToggleLineNumbers | Action::CommandPalette | Action::QuickOpen => "View",
            Action::GlobalSearch | Action::Find | Action::Replace | Action::FindNext | Action::FindPrevious => "Search",
            _ => "Edit",
        }
    }
}
