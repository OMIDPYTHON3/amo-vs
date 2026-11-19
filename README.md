# AMO VS

A professional, cross-platform terminal code editor that recreates the
Visual Studio Code experience with terminal-based rendering. AMO VS is a real
editor — not a mockup — built with Rust, Ratatui, Crossterm, Ropey,
tree-sitter, Tokio and git2.

## Features

### Interface
- VS Code Dark+ theme (plus a Light+ theme and TOML-configurable custom themes)
- Menu bar: File, Edit, Selection, View, Go, Run, Terminal, Help
- Activity bar (Explorer, Search, Source Control, Run and Debug)
- Collapsible file explorer with git decorations and open-editors section
- Multiple editor tabs with breadcrumbs, line numbers and a minimap
- Bottom panel with Problems, Output, Debug Console and a live terminal
- VS Code-style status bar (branch, changes, diagnostics, cursor, encoding, language)
- Command Palette (`Ctrl+P`, then `>`, or `F1`), quick open, find/replace and global search

### Editor engine
- Ropey-backed UTF-8 buffer with grouped undo/redo
- Cursor movement, word movement, selection, copy/cut/paste (OSC 52 + internal clipboard)
- Find and replace (case sensitivity, whole-word, replace one/all)
- Auto indentation, bracket auto-closing and bracket matching
- Tree-sitter syntax highlighting for Rust, JavaScript/TypeScript and Python;
  a refined built-in tokenizer covers C/C++, Java, Go, JSON, HTML, CSS,
  Markdown, TOML, Shell, YAML and SQL (including JSON keys, TOML tables,
  Markdown headings, C preprocessor directives, Rust macros and string escapes)
- Intelligent autocomplete: context-aware identifiers from the buffer and
  workspace, language keywords and best-effort member access after `.` with
  signature hints (configurable; LSP-ready item interface)
- Nine complete built-in themes with an in-app color-theme picker and live
  preview; selection is remembered between sessions
- Real navigable minimap with syntax-aware colors and a viewport highlight
  (click/drag to jump, wheel to page; automatically hidden on narrow screens)
- VS Code Seti-inspired file icon theme with Safe and Nerd Font glyph sets
- Explorer context menu (right-click / Shift+F10) for file operations
- Polished Welcome page with recent-project history and shortcuts
- Syntax-error diagnostics in the Problems panel
- Autosave and external-file-change detection
- Mouse support (clicks, double-click word selection, drag selection, wheel scroll)

### Files and tools
- File browser for opening files/folders, create/rename/delete files and folders
- Search across the workspace (respects `.gitignore`)
- Git status, branch/ahead/behind, commit from the Source Control view
- Integrated terminal with a **real PTY** (portable-pty / ConPTY on Windows),
  shell detection, command history and multiple sessions
- Run the active file in the integrated terminal (Rust, Python, JS, Go, Shell, Java)

## Requirements

- Rust 1.74 or newer (edition 2021)
- A C compiler and CMake (for tree-sitter grammars and libgit2)
- A UTF-8 terminal; Nerd Fonts are optional (the shipped icons are single-column)

## Build

```bash
./build.sh release        # builds target/release/amo-vs and copies it to dist/
./build.sh debug
./build.sh all            # cross-compile all supported targets (needs rustup targets)
```

On Windows PowerShell:

```powershell
.\build.ps1 -Configuration Release
```

Build directly with Cargo:

```bash
cargo build --release
cargo test
```

## Run

```bash
amo-vs                       # empty workspace
amo-vs /path/to/project      # open a folder
amo-vs src/main.rs           # open a file
amo-vs --no-mouse            # disable mouse capture
```

### Themes and completion

Nine complete palettes ship with AMO VS: Dark+, Light+, Dracula, Nord,
Tokyo Night, Catppuccin Mocha, Monokai, GitHub Dark and One Dark Pro. Full
TOML definitions live in `themes/`. Open `Preferences: Color Theme` from the
Command Palette (or the View menu) to filter, live-preview and apply a theme;
the choice is saved to `settings.toml`. `Preferences: Next/Previous Color Theme`
cycles instantly.

Completion is enabled by default. Type at least two identifier characters to
see suggestions, use `↑`/`Down` to navigate, `Tab`/`Enter` to accept and `Esc`
to dismiss. Suggestions come from the current buffer, the workspace and
language keywords; after `.` AMO VS attempts type-aware member completion.
Tune it with `autocomplete`, `autocomplete_max_items` and
`autocomplete_workspace` in `settings.toml`.

### Exit and terminal safety

`Ctrl+Esc` (or `Ctrl+Q`) exits from any panel, dialog or overlay. If any file is
modified, a dialog offers **Save All and Exit**, **Exit Without Saving** and
**Cancel**; otherwise the editor exits immediately. On terminal emulators that
cannot report Ctrl+Esc separately, a bare `Esc` also opens the dialog when no
dialog or selection is active (the integrated terminal keeps `Esc` for shell
programs). On exit or SIGINT/SIGTERM/SIGHUP, AMO VS kills PTY sessions and
atomically restores raw mode, the alternate screen, mouse reporting and cursor
visibility. A panic hook performs the same restoration.

Configuration lives in your platform config directory (`~/.config/amo-vs`
on Linux, `~/Library/Application Support/amo-vs` on macOS,
`%APPDATA%\amo-vs` on Windows). A commented `settings.toml` is generated on
first run. Custom themes can be placed in `<config dir>/themes/<name>.toml`.

## Keyboard shortcuts

| Shortcut | Action |
| --- | --- |
| `Ctrl+P` | Go to File (type `>` for commands) |
| `F1` / `Ctrl+Shift+P` | Command Palette |
| `Ctrl+O` / `Ctrl+K Ctrl+O` | Open File / Folder |
| `Ctrl+N` / `Ctrl+W` | New / Close editor |
| `Ctrl+S` / `Ctrl+Shift+S` | Save / Save As |
| `Ctrl+F` / `Ctrl+H` | Find / Replace |
| `Ctrl+Shift+F` | Find in Files |
| `Ctrl+G` | Go to Line |
| `Ctrl+/` | Toggle line comment |
| `Ctrl+Z` / `Ctrl+Y` | Undo / Redo |
| `Ctrl+B` / `Ctrl+J` | Toggle side bar / panel |
| Ctrl+Grave / Ctrl+Shift+Grave | Toggle / new terminal |
| `Ctrl+Esc` | Exit globally from any panel, dialog or popup |
| `Tab` / `Enter` | Accept the highlighted completion |
| `Ctrl+P` then `>` | Command Palette (use "Preferences: Color Theme") |
| `Ctrl+Q` | Alternative exit shortcut (useful in Termux) |
| Right-click / `Shift+F10` | Explorer context menu |
| `Alt+Z` | Toggle word wrap |
| `F5` | Run active file |
| `F2` / `Delete` | Rename / delete in explorer |

## Architecture

```
src/
  app/         application state, actions, key handling, command registry
  editor/      Ropey buffer, cursor/view, syntax highlighting, brackets
  ui/          Ratatui layout and widgets for every VS Code region
  workspace/   file tree, workspace search, file watcher
  terminal/    PTY sessions, ANSI screen emulator, shell helpers
  services/    configuration, clipboard, fuzzy matching, git
  platform/    Unix/Windows shell detection and paths
  themes/      Dark+ and Light+ palettes
```

## Cross-platform notes

- **Linux x86_64 / ARM64:** native PTY via `/dev/ptmx`; Termux is detected and
  uses its own shell paths.
- **macOS:** native PTY; config in `~/Library/Application Support/amo-vs`.
- **Windows x86_64:** ConPTY through `portable-pty`, with PowerShell/PowerShell 7/cmd detection.

The interface adapts to small screens: the sidebar and minimap are hidden on
narrow terminals and the panel is clamped to the available height.

## License

MIT
