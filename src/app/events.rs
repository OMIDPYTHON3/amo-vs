use super::{Action, App, ConfirmStyle, Focus, Overlay, PanelTab, PromptKind, SideBarView};
use crate::ui::layout;
use crate::ui::sidebar::{explorer_rows, ExplorerRow};
use crate::app::MessageKind;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use std::path::PathBuf;
use std::time::Instant;

/// Central global keyboard interceptor. Called by the event loop for every key
/// before any component (editor, terminal, popup, dialog, picker, ...) sees it.
/// This guarantees Ctrl+Esc can never be swallowed by a focused widget.
pub fn global_shortcut(app: &App, code: KeyCode, mods: KeyModifiers) -> Option<Action> {
    let kb = &app.config.keybindings;
    if kb.matches(&kb.exit.clone(), code, mods) || kb.matches(&kb.quit.clone(), code, mods) {
        return Some(Action::Quit);
    }
    if kb.matches(&kb.command_palette.clone(), code, mods) {
        return Some(Action::CommandPalette);
    }
    // Portable fallback: most terminals cannot report Ctrl+Esc separately
    // from Esc. Use a bare Esc when nothing else needs to dismiss it, except
    // in the terminal where Esc belongs to shell programs.
    if code == KeyCode::Esc && mods.is_empty()
        && app.overlay.is_none()
        && !app.completion.visible
        && app.focus != Focus::Terminal
        && !app.active_editor().map(|e| e.view.has_selection()).unwrap_or(false)
    {
        return Some(Action::Quit);
    }
    None
}

pub fn handle(app: &mut App, event: Event) {
    match event {
        Event::Key(key) if key.kind != KeyEventKind::Release => {
            if let Some(action) = global_shortcut(app, key.code, key.modifiers) {
                // Ctrl+Esc / Ctrl+Q always win, even with a popup visible.
                app.execute(action);
            } else if key.code == KeyCode::Esc && key.modifiers.is_empty()
                && app.completion.visible && app.overlay.is_none() {
                app.dismiss_completion();
            } else {
                key_event(app, key);
            }
        }
        Event::Mouse(mouse) => mouse_event(app, mouse),
        Event::Resize(cols, rows) => app.set_terminal_size(cols as usize, rows as usize),
        Event::Paste(text) => paste_event(app, &text),
        _ => {}
    }
}

fn paste_event(app: &mut App, text: &str) {
    let config = app.config.clone();
    match app.overlay {
        Some(Overlay::CommandPalette) | Some(Overlay::QuickOpen) => {
            let byte = char_to_byte(&app.palette.query, app.palette.query.chars().count());
            app.palette.query.insert_str(byte, text);
            app.update_palette();
        }
        Some(Overlay::Find) | Some(Overlay::Replace) => {
            let field = if app.overlay == Some(Overlay::Replace) { app.find.field } else { 0 };
            let s = if field == 0 { &mut app.find.query } else { &mut app.find.replacement };
            let at = char_to_byte(s, s.chars().count());
            s.insert_str(at, text);
            app.find_in_editor();
        }
        Some(Overlay::Prompt) => {
            if let Some(prompt) = &mut app.prompt {
                let at = char_to_byte(&prompt.value, prompt.cursor);
                prompt.value.insert_str(at, text);
                prompt.cursor += text.chars().count();
            }
        }
        Some(Overlay::GlobalSearch) => {
            let s = if app.search.field == 0 { &mut app.search.query } else { &mut app.search.replacement };
            let at = char_to_byte(s, s.chars().count());
            s.insert_str(at, text);
        }
        _ if app.focus == Focus::Editor => {
            if let Some(editor) = app.active_editor_mut() { editor.insert_text(text, &config); }
            app.update_completion();
        }
        _ if app.focus == Focus::Terminal => {
            if let Some(session) = app.terminal.active_mut() { session.write_bytes(text.as_bytes()); }
        }
        _ => {}
    }
}

fn key_event(app: &mut App, key: KeyEvent) {
    let code = key.code;
    let mods = key.modifiers;
    let ctrl = mods.contains(KeyModifiers::CONTROL);
    let alt = mods.contains(KeyModifiers::ALT);
    let shift = mods.contains(KeyModifiers::SHIFT);

    // Completion popup navigation (it is not a modal overlay, so normal
    // typing keeps flowing while it is visible).
    if app.completion.visible && app.overlay.is_none() {
        match code {
            KeyCode::Up => { app.completion_move(-1); return; }
            KeyCode::Down => { app.completion_move(1); return; }
            KeyCode::Tab | KeyCode::Enter => { app.accept_completion(); return; }
            KeyCode::Char(_) if !ctrl && !alt => { /* fall through: type & refresh */ }
            _ => app.completion.clear(),
        }
    }

    // Overlays have priority (global shortcuts already handled in `handle`).
    if app.overlay == Some(Overlay::Menu) || app.menu.open.is_some() {
        menu_key(app, code, ctrl);
        return;
    }
    if app.overlay == Some(Overlay::ThemePicker) { theme_picker_key(app, key); return; }
    if app.overlay == Some(Overlay::Confirm) {
        confirm_key(app, code);
        return;
    }
    if app.overlay == Some(Overlay::ContextMenu) {
        context_menu_key(app, code);
        return;
    }
    if app.overlay == Some(Overlay::Prompt) { prompt_key(app, key); return; }
    if app.overlay == Some(Overlay::FileBrowser) { browser_key(app, key); return; }
    if app.overlay == Some(Overlay::Help) { app.overlay = None; return; }
    if app.overlay == Some(Overlay::CommandPalette) || app.overlay == Some(Overlay::QuickOpen) {
        palette_key(app, key);
        return;
    }
    if app.overlay == Some(Overlay::Find) || app.overlay == Some(Overlay::Replace) {
        find_key(app, key);
        return;
    }
    if app.overlay == Some(Overlay::GlobalSearch) { search_key(app, key); return; }

    // Ctrl+K chord starter.
    if ctrl && code == KeyCode::Char('k') && !alt && !shift {
        app.chord_k = true;
        return;
    }

    // Ctrl+K chord.
    if app.chord_k {
        app.chord_k = false;
        match code {
            KeyCode::Char('o' | 'O') => { app.execute(Action::OpenFolder); return; }
            KeyCode::Char('s' | 'S') => { app.execute(Action::SaveAll); return; }
            KeyCode::Char('c' | 'C') => { app.execute(Action::ToggleComment); return; }
            _ => {}
        }
    }

    // Terminal focus: most keys go straight to the shell.
    if app.focus == Focus::Terminal {
        if let Some(action) = terminal_escape(code, mods) {
            app.execute(action);
            return;
        }
        if code == KeyCode::Enter {
            let limit = app.config.terminal_history_size;
            if let Some(session) = app.terminal.active_mut() { session.submit(limit); }
            return;
        }
        if let Some(session) = app.terminal.active_mut() { session.send_key(code, mods); }
        return;
    }

    if let Some(action) = global_action(code, mods, app.focus) {
        app.execute(action);
        return;
    }

    match app.focus {
        Focus::Editor => editor_key(app, key),
        Focus::Explorer => explorer_key(app, code, mods),
        Focus::Panel => panel_key(app, code),
        Focus::Terminal => {}
    }
}

fn terminal_escape(code: KeyCode, mods: KeyModifiers) -> Option<Action> {
    let ctrl = mods.contains(KeyModifiers::CONTROL);
    let shift = mods.contains(KeyModifiers::SHIFT);
    match (code, ctrl, shift) {
        (KeyCode::Char('p'), true, true) => Some(Action::CommandPalette),
        (KeyCode::Char('p'), true, false) => Some(Action::QuickOpen),
        (KeyCode::Char('`'), true, _) => Some(Action::ToggleTerminal),
        (KeyCode::Char('b'), true, false) => Some(Action::ToggleSidebar),
        (KeyCode::Tab, true, _) => Some(Action::NextTab),
        (KeyCode::F(1), _, _) => Some(Action::CommandPalette),
        _ => None,
    }
}

fn theme_picker_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.theme_picker_cancel(),
        KeyCode::Enter => app.theme_picker_accept(),
        KeyCode::Up => app.theme_picker_move(-1),
        KeyCode::Down => app.theme_picker_move(1),
        KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => app.theme_picker_move(1),
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => app.theme_picker_move(-1),
        KeyCode::Backspace => {
            if let Some(p) = &mut app.theme_picker { p.query.pop(); p.selected = 0; }
            app.update_theme_picker();
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT) => {
            if let Some(p) = &mut app.theme_picker { p.query.push(c); p.selected = 0; }
            app.update_theme_picker();
        }
        _ => {}
    }
}

fn context_menu_key(app: &mut App, code: KeyCode) {
    let Some(menu) = app.context_menu.as_ref() else { return; };
    let count = menu.actions.len();
    let selected = menu.selected;
    match code {
        KeyCode::Up => app.context_menu.as_mut().unwrap().selected = (selected + count - 1) % count,
        KeyCode::Down => app.context_menu.as_mut().unwrap().selected = (selected + 1) % count,
        KeyCode::Enter => app.execute_context_action(),
        KeyCode::Esc => { app.context_menu = None; app.overlay = None; }
        _ => {}
    }
}

fn confirm_key(app: &mut App, code: KeyCode) {
    let Some(confirm) = app.confirm.as_mut() else { return; };
    let count = confirm.option_count();
    let style = confirm.style;
    match code {
        KeyCode::Left | KeyCode::Up => {
            let selected = &mut app.confirm.as_mut().unwrap().selected;
            *selected = (*selected + count - 1) % count;
        }
        KeyCode::Right | KeyCode::Down => {
            let selected = &mut app.confirm.as_mut().unwrap().selected;
            *selected = (*selected + 1) % count;
        }
        KeyCode::Enter => {
            let selected = app.confirm.as_ref().unwrap().selected;
            apply_confirm_choice(app, style, selected);
        }
        KeyCode::Char('1') if style == ConfirmStyle::Exit => app.execute(Action::SaveAllAndExit),
        KeyCode::Char('2') if style == ConfirmStyle::Exit => app.execute(Action::ExitWithoutSaving),
        KeyCode::Char('3') if style == ConfirmStyle::Exit => app.execute(Action::CancelExit),
        KeyCode::Char('y' | 'Y') => app.confirm_accept(),
        KeyCode::Char('n' | 'N') | KeyCode::Esc => app.execute(Action::CancelExit),
        _ => {}
    }
}

fn apply_confirm_choice(app: &mut App, style: ConfirmStyle, selected: usize) {
    match style {
        ConfirmStyle::YesNo => app.confirm_accept(),
        ConfirmStyle::Exit => match selected {
            0 => app.execute(Action::SaveAllAndExit),
            1 => app.execute(Action::ExitWithoutSaving),
            _ => app.execute(Action::CancelExit),
        },
    }
}

fn menu_key(app: &mut App, code: KeyCode, ctrl: bool) {
    let _ = ctrl;
    if app.menu.open.is_none() && app.overlay == Some(Overlay::Menu) {
        if code == KeyCode::Esc { app.overlay = None; app.menu.open = None; }
        return;
    }
    let Some(index) = app.menu.open else { return; };
    let entries = crate::app::command::menu_entries(index);
    match code {
        KeyCode::Esc => { app.menu.open = None; app.overlay = None; }
        KeyCode::Left => {
            app.menu.open = Some(if index == 0 { 7 } else { index - 1 });
            app.menu.selected = 0;
        }
        KeyCode::Right => {
            app.menu.open = Some((index + 1) % 8);
            app.menu.selected = 0;
        }
        KeyCode::Down => {
            if !entries.is_empty() {
                app.menu.selected = (app.menu.selected + 1) % entries.len();
                while entries.get(app.menu.selected).map(|e| e.separator).unwrap_or(false) {
                    app.menu.selected = (app.menu.selected + 1) % entries.len();
                }
            }
        }
        KeyCode::Up => {
            if !entries.is_empty() {
                app.menu.selected = (app.menu.selected + entries.len() - 1) % entries.len();
                while entries.get(app.menu.selected).map(|e| e.separator).unwrap_or(false) {
                    app.menu.selected = (app.menu.selected + entries.len() - 1) % entries.len();
                }
            }
        }
        KeyCode::Enter => {
            if let Some(action) = entries.get(app.menu.selected).and_then(|e| e.action.clone()) {
                app.menu.open = None;
                app.overlay = None;
                app.execute(action);
            }
        }
        KeyCode::Char(c) => {
            if let Some(pos) = entries.iter().position(|e| {
                e.label.chars().next().map(|x| x.eq_ignore_ascii_case(&c)).unwrap_or(false)
            }) {
                app.menu.selected = pos;
            }
        }
        _ => {}
    }
}

fn prompt_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Enter => app.prompt_accept(),
        KeyCode::Esc => { app.prompt = None; app.overlay = None; }
        KeyCode::Backspace => {
            if let Some(prompt) = &mut app.prompt {
                if prompt.cursor > 0 {
                    let byte = char_to_byte(&prompt.value, prompt.cursor);
                    let prev = prompt.value[..byte].chars().next_back().map(|c| c.len_utf8()).unwrap_or(0);
                    prompt.value.remove(byte - prev);
                    prompt.cursor -= 1;
                }
            }
        }
        KeyCode::Delete => {
            if let Some(prompt) = &mut app.prompt {
                let byte = char_to_byte(&prompt.value, prompt.cursor);
                if byte < prompt.value.len() {
                    let len = prompt.value[byte..].chars().next().map(|c| c.len_utf8()).unwrap_or(0);
                    prompt.value.drain(byte..byte + len);
                }
            }
        }
        KeyCode::Left => {
            if let Some(prompt) = &mut app.prompt { prompt.cursor = prompt.cursor.saturating_sub(1); }
        }
        KeyCode::Right => {
            if let Some(prompt) = &mut app.prompt {
                prompt.cursor = (prompt.cursor + 1).min(prompt.value.chars().count());
            }
        }
        KeyCode::Home => { if let Some(p) = &mut app.prompt { p.cursor = 0; } }
        KeyCode::End => { if let Some(p) = &mut app.prompt { p.cursor = p.value.chars().count(); } }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(p) = &mut app.prompt { p.value.clear(); p.cursor = 0; }
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT) => {
            if let Some(prompt) = &mut app.prompt {
                let byte = char_to_byte(&prompt.value, prompt.cursor);
                prompt.value.insert(byte, c);
                prompt.cursor += 1;
            }
        }
        _ => {}
    }
}

fn browser_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => { app.file_browser = None; app.overlay = None; }
        KeyCode::Enter => app.browser_enter(),
        KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(browser) = &app.file_browser {
                if browser.mode == crate::app::FileBrowserMode::OpenFolder {
                    let dir = browser.dir.clone();
                    app.file_browser = None;
                    app.overlay = None;
                    if let Err(e) = app.open_workspace(dir) {
                        app.notify(format!("Open folder failed: {}", e), MessageKind::Error);
                    }
                }
            }
        }
        KeyCode::Backspace => {
            if let Some(browser) = &mut app.file_browser {
                if browser.filter.is_empty() {
                    let _ = browser;
                    app.browser_up();
                } else {
                    browser.filter.pop();
                    crate::app::refresh_browser_for(app);
                }
            }
        }
        KeyCode::Up => {
            if let Some(browser) = &mut app.file_browser {
                browser.selected = browser.selected.saturating_sub(1);
            }
        }
        KeyCode::Down => {
            if let Some(browser) = &mut app.file_browser {
                browser.selected = (browser.selected + 1).min(browser.entries.len().saturating_sub(1));
            }
        }
        KeyCode::Char('/') => {
            if let Some(browser) = &mut app.file_browser {
                if browser.filter.is_empty() {
                    browser.dir = PathBuf::from("/");
                    crate::app::refresh_browser_for(app);
                    return;
                }
            }
            append_browser_char(app, '/');
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => append_browser_char(app, c),
        _ => {}
    }
}

fn append_browser_char(app: &mut App, c: char) {
    if let Some(browser) = &mut app.file_browser {
        browser.filter.push(c);
        browser.selected = 0;
    }
    crate::app::refresh_browser_for(app);
}

fn palette_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => { app.overlay = None; }
        KeyCode::Down | KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if !app.palette.items.is_empty() {
                app.palette.selected = (app.palette.selected + 1) % app.palette.items.len();
                ensure_palette_visible(app);
            }
        }
        KeyCode::Up | KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if !app.palette.items.is_empty() {
                app.palette.selected = (app.palette.selected + app.palette.items.len() - 1) % app.palette.items.len();
                ensure_palette_visible(app);
            }
        }
        KeyCode::Down => {
            if !app.palette.items.is_empty() {
                app.palette.selected = (app.palette.selected + 1) % app.palette.items.len();
                ensure_palette_visible(app);
            }
        }
        KeyCode::Up => {
            if !app.palette.items.is_empty() {
                app.palette.selected = (app.palette.selected + app.palette.items.len() - 1) % app.palette.items.len();
                ensure_palette_visible(app);
            }
        }
        KeyCode::Enter => palette_accept(app),
        KeyCode::Backspace => {
            app.palette.query.pop();
            app.palette.selected = 0;
            app.palette.scroll = 0;
            app.update_palette();
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT) => {
            app.palette.query.push(c);
            app.palette.selected = 0;
            app.palette.scroll = 0;
            app.update_palette();
        }
        _ => {}
    }
}

fn ensure_palette_visible(app: &mut App) {
    let visible = 14usize;
    let selected = app.palette.selected;
    if selected < app.palette.scroll { app.palette.scroll = selected; }
    else if selected >= app.palette.scroll + visible { app.palette.scroll = selected - visible + 1; }
}

fn palette_accept(app: &mut App) {
    let Some(item) = app.palette.items.get(app.palette.selected).cloned() else { return; };
    app.overlay = None;
    if let Some(action) = item.action { app.execute(action); }
    else if let Some(path) = item.path {
        if let Err(e) = app.open_path(&path) {
            app.notify(format!("Open failed: {}", e), MessageKind::Error);
        }
    }
}

fn find_key(app: &mut App, key: KeyEvent) {
    let code = key.code;
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    match code {
        KeyCode::Esc => { app.overlay = None; app.focus = Focus::Editor; }
        KeyCode::Tab if app.overlay == Some(Overlay::Replace) => {
            app.find.field = (app.find.field + 1) % 2;
        }
        KeyCode::Enter => {
            if app.find.field == 1 && app.overlay == Some(Overlay::Replace) {
                replace_current(app);
            } else {
                let next = if app.find.current + 1 < app.find.matches.len() { app.find.current + 1 } else { 0 };
                app.goto_match(next);
            }
        }
        KeyCode::F(3) => {
            app.find_in_editor();
            if shift {
                let prev = if app.find.current == 0 { app.find.matches.len().saturating_sub(1) } else { app.find.current - 1 };
                app.goto_match(prev);
            } else {
                let next = if app.find.current + 1 < app.find.matches.len() { app.find.current + 1 } else { 0 };
                app.goto_match(next);
            }
        }
        KeyCode::Up => {
            let prev = if app.find.current == 0 { app.find.matches.len().saturating_sub(1) } else { app.find.current - 1 };
            app.goto_match(prev);
        }
        KeyCode::Down => {
            let next = if app.find.current + 1 < app.find.matches.len() { app.find.current + 1 } else { 0 };
            app.goto_match(next);
        }
        KeyCode::Backspace => {
            let s = if app.find.field == 1 && app.overlay == Some(Overlay::Replace) { &mut app.find.replacement } else { &mut app.find.query };
            s.pop();
            if app.find.field == 0 { app.find_in_editor(); }
        }
        KeyCode::Char('c') if alt => { app.find.case_sensitive = !app.find.case_sensitive; app.find_in_editor(); }
        KeyCode::Char('w') if alt => { app.find.whole_word = !app.find.whole_word; app.find_in_editor(); }
        KeyCode::Char('r') if alt => replace_current(app),
        KeyCode::Char('a') if alt => replace_all_in_editor(app),
        KeyCode::Char(c) if !ctrl && !alt => {
            let s = if app.find.field == 1 && app.overlay == Some(Overlay::Replace) { &mut app.find.replacement } else { &mut app.find.query };
            s.push(c);
            if app.find.field == 0 { app.find_in_editor(); }
        }
        _ => {}
    }
}

fn replace_current(app: &mut App) {
    app.find_in_editor();
    let Some(&(start, end)) = app.find.matches.get(app.find.current) else { return; };
    let replacement = app.find.replacement.clone();
    if let Some(editor) = app.active_editor_mut() {
        editor.buffer.replace_range(start, end, &replacement);
        editor.view.cursor = start + replacement.chars().count();
        editor.view.anchor = None;
    }
    app.find_in_editor();
}

fn replace_all_in_editor(app: &mut App) {
    let query = app.find.query.clone();
    let replacement = app.find.replacement.clone();
    let case = app.find.case_sensitive;
    let word = app.find.whole_word;
    if let Some(editor) = app.active_editor_mut() {
        let count = editor.replace_all(&query, &replacement, case, word);
        app.notify(format!("Replaced {} occurrence(s)", count), MessageKind::Info);
    }
    app.find_in_editor();
}

fn search_key(app: &mut App, key: KeyEvent) {
    let code = key.code;
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    match code {
        KeyCode::Esc => { app.overlay = None; }
        KeyCode::Tab => {
            app.search.show_replace = true;
            app.search.field = (app.search.field + 1) % 2;
        }
        KeyCode::Enter => {
            if app.search.field == 2 || (!app.search.results.is_empty() && ctrl) {
                open_search_result(app);
            } else {
                app.perform_search();
            }
        }
        KeyCode::Up | KeyCode::Down => {
            app.search.field = 2;
            if !app.search.results.is_empty() {
                if code == KeyCode::Down {
                    app.search.selected = (app.search.selected + 1) % app.search.results.len();
                } else {
                    app.search.selected = (app.search.selected + app.search.results.len() - 1) % app.search.results.len();
                }
            }
        }
        KeyCode::Backspace => {
            let s = if app.search.field == 1 { &mut app.search.replacement } else { &mut app.search.query };
            s.pop();
        }
        KeyCode::Char('c') if alt => { app.search.case_sensitive = !app.search.case_sensitive; app.perform_search(); }
        KeyCode::Char('w') if alt => { app.search.whole_word = !app.search.whole_word; app.perform_search(); }
        KeyCode::Char('a') if alt => replace_all_files(app),
        KeyCode::Char(c) if !ctrl && !alt => {
            let field = app.search.field.min(1);
            if field == 0 { app.search.query.push(c); } else { app.search.replacement.push(c); }
        }
        _ => {}
    }
}

fn open_search_result(app: &mut App) {
    let Some(result) = app.search.results.get(app.search.selected).cloned() else { return; };
    if app.open_path(&result.path).is_ok() {
        if let Some(editor) = app.active_editor_mut() {
            let line = result.line.saturating_sub(1).min(editor.buffer.len_lines().saturating_sub(1));
            editor.view.cursor = editor.buffer.line_to_char(line) + result.col.saturating_sub(1);
            editor.view.anchor = None;
        }
        app.overlay = None;
        app.focus = Focus::Editor;
    }
}

fn replace_all_files(app: &mut App) {
    let results = app.search.results.clone();
    let replacement = app.search.replacement.clone();
    let mut paths: Vec<PathBuf> = results.iter().map(|r| r.path.clone()).collect();
    paths.sort();
    paths.dedup();
    let mut count = 0;
    for path in paths {
        let Ok(bytes) = std::fs::read(&path) else { continue; };
        let updated = String::from_utf8_lossy(&bytes).to_string();
        let mut result = String::new();
        let mut cursor = 0usize;
        let lower = updated.to_lowercase();
        let needle = app.search.query.to_lowercase();
        if !needle.is_empty() {
            while let Some(pos) = lower[cursor..].find(&needle) {
                let start = cursor + pos;
                let end = start + needle.len();
                result.push_str(&updated[cursor..start]);
                result.push_str(&replacement);
                count += 1;
                cursor = end;
                if cursor > updated.len() { break; }
            }
            result.push_str(&updated[cursor..]);
            if std::fs::write(&path, result).is_ok() { /* written */ }
        }
    }
    app.notify(format!("Replaced in {} file(s), {} occurrence(s)", paths_len(&results), count), MessageKind::Info);
    app.perform_search();
}

fn paths_len(results: &[crate::workspace::SearchResult]) -> usize {
    let mut paths: Vec<_> = results.iter().map(|r| r.path.clone()).collect();
    paths.sort();
    paths.dedup();
    paths.len()
}

fn editor_key(app: &mut App, key: KeyEvent) {
    let config = app.config.clone();
    let code = key.code;
    let mods = key.modifiers;
    let ctrl = mods.contains(KeyModifiers::CONTROL);
    let alt = mods.contains(KeyModifiers::ALT);
    let shift = mods.contains(KeyModifiers::SHIFT);

    // Welcome page: navigate actions instead of editing the scratch buffer.
    if app.active_editor().map(|e| e.is_welcome).unwrap_or(false) {
        match code {
            KeyCode::Up => {
                app.welcome_selected = app.welcome_selected.saturating_sub(1);
                return;
            }
            KeyCode::Down => {
                let count = app.welcome_actions().len();
                app.welcome_selected = (app.welcome_selected + 1).min(count.saturating_sub(1));
                return;
            }
            KeyCode::Enter => {
                let index = app.welcome_selected;
                app.activate_welcome(index);
                return;
            }
            KeyCode::Char(_) | KeyCode::Backspace | KeyCode::Delete | KeyCode::Tab
                if !ctrl && !alt => return,
            _ => {}
        }
    }

    let mut handled = false;
    let mut refresh_completion = false;
    if let Some(editor) = app.active_editor_mut() {
        match code {
            KeyCode::Char(c) if !ctrl && !alt => {
                editor.insert_text(&c.to_string(), &config);
                handled = true; refresh_completion = true;
            }
            KeyCode::Enter => { editor.newline(&config); handled = true; refresh_completion = true; }
            KeyCode::Backspace => { editor.backspace(&config); handled = true; refresh_completion = true; }
            KeyCode::Delete => { editor.delete(); handled = true; refresh_completion = true; }
            KeyCode::Tab => { if shift { editor.outdent(&config); } else { editor.indent(&config); } handled = true; }
            KeyCode::Left => {
                if ctrl { editor.view.move_word_left(&editor.buffer, shift); } else { editor.view.move_left(&editor.buffer, shift); }
                handled = true;
            }
            KeyCode::Right => {
                if ctrl { editor.view.move_word_right(&editor.buffer, shift); } else { editor.view.move_right(&editor.buffer, shift); }
                handled = true;
            }
            KeyCode::Up => { editor.view.move_up(&editor.buffer, shift); handled = true; }
            KeyCode::Down => { editor.view.move_down(&editor.buffer, shift); handled = true; }
            KeyCode::Home => { if ctrl { editor.view.move_doc_start(shift); } else { editor.view.move_home(&editor.buffer, shift); } handled = true; }
            KeyCode::End => { if ctrl { editor.view.move_doc_end(&editor.buffer, shift); } else { editor.view.move_end(&editor.buffer, shift); } handled = true; }
            KeyCode::PageUp => { editor.view.move_page_up(&editor.buffer, 20, shift); handled = true; }
            KeyCode::PageDown => { editor.view.move_page_down(&editor.buffer, 20, shift); handled = true; }
            _ => {}
        }
    }
    if refresh_completion { app.update_completion(); }
    else if handled { app.completion.clear(); }
    if handled { return; }

    if let Some(action) = global_action(code, mods, Focus::Editor) {
        app.execute(action);
    }
}

fn explorer_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    let rows = explorer_rows(app);
    match code {
        KeyCode::Up => {
            app.explorer_selected = app.explorer_selected.saturating_sub(1);
            ensure_explorer_visible(app);
        }
        KeyCode::Down => {
            app.explorer_selected = (app.explorer_selected + 1).min(rows.len().saturating_sub(1));
            ensure_explorer_visible(app);
        }
        KeyCode::Enter => match rows.get(app.explorer_selected) {
            Some(ExplorerRow::Editor(i)) => { app.active = *i; app.focus = Focus::Editor; }
            Some(ExplorerRow::Tree(entry)) => {
                let path = entry.path.clone();
                if entry.is_dir { app.workspace.as_mut().map(|w| w.toggle(&path)); }
                else if let Err(e) = app.open_path(&path) { app.notify(format!("Open failed: {}", e), MessageKind::Error); }
            }
            None => {}
        },
        KeyCode::Left | KeyCode::Right => {
            if let Some(ExplorerRow::Tree(entry)) = rows.get(app.explorer_selected) {
                let path = entry.path.clone();
                let should_toggle = (code == KeyCode::Left && entry.expanded && entry.is_dir)
                    || (code == KeyCode::Right && !entry.expanded && entry.is_dir);
                if should_toggle { app.workspace.as_mut().map(|w| w.toggle(&path)); }
            }
        }
        KeyCode::F(2) => app.execute(Action::ExplorerRename),
        KeyCode::Delete => app.execute(Action::ExplorerDelete),
        KeyCode::F(10) => {
            if let Some(path) = app.selected_path() {
                let is_dir = path.is_dir();
                let (w, h) = app.terminal_size;
                app.open_context_menu(path, is_dir, app.sidebar_x() + 4, app.sidebar_y() + app.explorer_selected as u16 + 3);
                let _ = (w, h);
            }
        }
        _ => {
            if let Some(action) = global_action(code, mods, Focus::Explorer) { app.execute(action); }
        }
    }
}

fn ensure_explorer_visible(app: &mut App) {
    let visible = 16usize;
    let selected = app.explorer_selected;
    if selected < app.explorer_scroll { app.explorer_scroll = selected; }
    else if selected >= app.explorer_scroll + visible { app.explorer_scroll = selected - visible + 1; }
}

fn panel_key(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Up => app.panel.scroll = app.panel.scroll.saturating_sub(1),
        KeyCode::Down => app.panel.scroll += 1,
        KeyCode::Tab => app.execute(Action::NextPanelTab),
        _ => {}
    }
}

fn global_action(code: KeyCode, mods: KeyModifiers, focus: Focus) -> Option<Action> {
    let ctrl = mods.contains(KeyModifiers::CONTROL);
    let shift = mods.contains(KeyModifiers::SHIFT);
    let alt = mods.contains(KeyModifiers::ALT);

    if ctrl && !alt {
        match code {
            KeyCode::Char('s') => return Some(if shift { Action::SaveAs } else { Action::Save }),
            KeyCode::Char('o') => return Some(Action::OpenFile),
            KeyCode::Char('n') => return Some(Action::NewFile),
            KeyCode::Char('w') => return Some(Action::CloseTab),
            KeyCode::Char('q') => return Some(Action::Quit),
            KeyCode::Char('p') => return Some(if shift { Action::CommandPalette } else { Action::QuickOpen }),
            KeyCode::Char('f') => return Some(if shift { Action::GlobalSearch } else { Action::Find }),
            KeyCode::Char('h') => return Some(Action::Replace),
            KeyCode::Char('g') => return Some(Action::GotoLine),
            KeyCode::Char('b') => return Some(Action::ToggleSidebar),
            KeyCode::Char('j') => return Some(Action::TogglePanel),
            KeyCode::Char('`') => return Some(if shift { Action::TerminalNew } else { Action::ToggleTerminal }),
            KeyCode::Char('z') => return Some(if shift { Action::Redo } else { Action::Undo }),
            KeyCode::Char('y') => return Some(Action::Redo),
            KeyCode::Char('a') => return Some(Action::SelectAll),
            KeyCode::Char('x') => return Some(Action::Cut),
            KeyCode::Char('c') => return Some(Action::Copy),
            KeyCode::Char('v') => return Some(Action::Paste),
            KeyCode::Char('/') => return Some(Action::ToggleComment),
            KeyCode::Char('d') if shift => return Some(Action::DuplicateLine),
            KeyCode::Char('k') if shift => return Some(Action::DeleteLine),
            KeyCode::Tab => return Some(if shift { Action::PreviousTab } else { Action::NextTab }),
            _ => {}
        }
    }
    match code {
        KeyCode::F(1) => return Some(Action::CommandPalette),
        KeyCode::F(5) => return Some(Action::RunActiveFile),
        KeyCode::Char('z') if alt => return Some(Action::ToggleWordWrap),
        KeyCode::F(2) if focus == Focus::Explorer => return Some(Action::ExplorerRename),
        _ => {}
    }
    None
}

fn mouse_event(app: &mut App, mouse: MouseEvent) {
    let (w, h) = app.terminal_size;
    if w == 0 || h == 0 { return; }
    let area = Rect { x: 0, y: 0, width: w, height: h };
    let r = layout::compute(app, area);
    let col = mouse.column;
    let row = mouse.row;

    match mouse.kind {
        MouseEventKind::ScrollUp => {
            if r.contains(r.minimap, col, row) {
                if let Some(editor) = app.active_editor_mut() {
                    editor.view.scroll_row = editor.view.scroll_row.saturating_sub((editor.buffer.len_lines() / r.minimap.height as usize).max(1));
                }
            } else if r.contains(r.editor_text, col, row) {
                if let Some(editor) = app.active_editor_mut() { editor.view.scroll_row = editor.view.scroll_row.saturating_sub(3); }
            } else if r.contains(r.panel_body, col, row) {
                app.panel.scroll = app.panel.scroll.saturating_sub(3);
            } else if r.contains(r.sidebar, col, row) {
                app.explorer_scroll = app.explorer_scroll.saturating_sub(3);
            }
        }
        MouseEventKind::ScrollDown => {
            if r.contains(r.minimap, col, row) {
                if let Some(editor) = app.active_editor_mut() {
                    let step = (editor.buffer.len_lines() / r.minimap.height as usize).max(1);
                    let max = editor.buffer.len_lines().saturating_sub(r.editor_text.height as usize);
                    editor.view.scroll_row = (editor.view.scroll_row + step).min(max);
                }
            } else if r.contains(r.editor_text, col, row) {
                if let Some(editor) = app.active_editor_mut() {
                    let max = editor.buffer.len_lines().saturating_sub(r.editor_text.height as usize);
                    editor.view.scroll_row = (editor.view.scroll_row + 3).min(max);
                }
            } else if r.contains(r.panel_body, col, row) {
                app.panel.scroll += 3;
            } else if r.contains(r.sidebar, col, row) {
                app.explorer_scroll += 3;
            }
        }
        MouseEventKind::Down(button) => {
            use crossterm::event::MouseButton;
            match button {
                MouseButton::Left => mouse_down(app, &r, col, row),
                MouseButton::Right => mouse_right_click(app, &r, col, row),
                _ => {}
            }
        }
        MouseEventKind::Drag(button) => {
            use crossterm::event::MouseButton;
            if button != MouseButton::Left { return; }
            if r.contains(r.minimap, col, row) {
                minimap_click(app, &r, row);
            } else if r.contains(r.editor_text, col, row) {
                if let Some(editor) = app.active_editor_mut() {
                    let line = editor.view.scroll_row + (row.saturating_sub(r.editor_text.y)) as usize;
                    if line < editor.buffer.len_lines() {
                        let col_index = (col.saturating_sub(r.editor_text.x)) as usize + editor.view.scroll_col;
                        let len = editor.buffer.line_content_len(line);
                        let idx = editor.buffer.line_to_char(line) + col_index.min(len);
                        editor.view.set_cursor(&editor.buffer, idx, true);
                    }
                }
            }
        }
        _ => {}
    }
}

fn mouse_right_click(app: &mut App, r: &layout::Rects, col: u16, row: u16) {
    if r.contains(r.sidebar, col, row) && app.sidebar_view == SideBarView::Explorer {
        let offset = if app.editors.is_empty() { 4u16 } else { (4 + app.editors.len() + 1) as u16 };
        if row > r.sidebar.y + offset {
            let rows = explorer_rows(app);
            let index = (row - r.sidebar.y - offset) as usize + app.explorer_scroll;
            if let Some(ExplorerRow::Tree(entry)) = rows.get(index) {
                app.explorer_selected = index;
                app.open_context_menu(entry.path.clone(), entry.is_dir, col, row);
                return;
            }
        }
        // Right-clicking empty explorer space creates a new file there.
        app.open_context_menu(app.cwd(), true, col, row);
    } else if r.contains(r.editor_text, col, row) {
        app.open_prompt(PromptKind::SaveAs);
    }
}

fn mouse_down(app: &mut App, r: &layout::Rects, col: u16, row: u16) {
    // Context menu has priority over everything else.
    if let Some(menu) = &app.context_menu {
        let screen = Rect { x: 0, y: 0, width: app.terminal_size.0, height: app.terminal_size.1 };
        let rect = crate::ui::overlays::context_menu_rect(menu, screen);
        if r.contains(rect, col, row) {
            let index = (row - rect.y - 1) as usize;
            if index < menu.actions.len() {
                app.context_menu.as_mut().unwrap().selected = index;
                app.execute_context_action();
            }
            return;
        }
        app.context_menu = None;
        app.overlay = None;
    }

    // Menu bar.
    if r.contains(r.menu, col, row) {
        let mut x = 0usize;
        for i in 0..8 {
            let name = crate::app::command::MENU_NAMES[i];
            if (x..x + name.len() + 4).contains(&(col as usize)) {
                app.menu.open = Some(i);
                app.menu.selected = 0;
                app.overlay = Some(Overlay::Menu);
                return;
            }
            x += name.len() + 4;
        }
        return;
    }

    // Activity bar.
    if r.contains(r.activity, col, row) {
        let views = [SideBarView::Explorer, SideBarView::Search, SideBarView::SourceControl, SideBarView::RunDebug];
        if let Some(i) = (0..4usize).find(|i| row == r.activity.y + 1 + (*i as u16) * 2) {
            let view = views[i];
            if app.sidebar_view == view && app.sidebar_visible { app.sidebar_visible = false; }
            else { app.sidebar_view = view; app.sidebar_visible = true; }
            app.focus = Focus::Explorer;
        } else if row >= r.activity.y + r.activity.height.saturating_sub(2) {
            app.execute(Action::CommandPalette);
        }
        return;
    }

    // Tabs (shares the exact geometry used for rendering).
    if r.contains(r.tabs, col, row) && !app.editors.is_empty() {
        for tab in crate::ui::tabs::tab_layout(app, r.tabs.width) {
            if (tab.x..tab.x + tab.width).contains(&(col - r.tabs.x)) {
                if let crate::ui::tabs::TabKind::Editor(index) = tab.kind {
                    app.active = index;
                    app.focus = Focus::Editor;
                }
                return;
            }
        }
        return;
    }

    // Sidebar.
    if r.contains(r.sidebar, col, row) {
        match app.sidebar_view {
            SideBarView::Explorer => {
                let rows = explorer_rows(app);
                let y_offset = if app.editors.is_empty() { 4u16 } else { (4 + app.editors.len() + 1) as u16 };
                if row > r.sidebar.y + y_offset {
                    let index = (row - r.sidebar.y - y_offset) as usize + app.explorer_scroll;
                    if let Some(ExplorerRow::Tree(entry)) = rows.get(index) {
                        let path = entry.path.clone();
                        app.explorer_selected = index;
                        if entry.is_dir { app.workspace.as_mut().map(|w| w.toggle(&path)); }
                        else { let _ = app.open_path(&path); }
                    }
                } else if let Some(ExplorerRow::Editor(i)) = rows.get((row.saturating_sub(r.sidebar.y + 2)) as usize) {
                    app.active = *i;
                    app.focus = Focus::Editor;
                }
            }
            SideBarView::Search => {
                let index = (row.saturating_sub(r.sidebar.y + 7)) as usize + app.search.scroll;
                if index < app.search.results.len() {
                    app.search.selected = index;
                    open_search_result(app);
                }
            }
            _ => {}
        }
        app.focus = Focus::Explorer;
        return;
    }

    // Minimap navigation: click positions the viewport proportionally.
    if r.contains(r.minimap, col, row) {
        minimap_click(app, r, row);
        app.focus = Focus::Editor;
        return;
    }

    // Editor.
    if r.contains(r.editor_text, col, row) {
        if app.completion.visible && app.overlay.is_none() {
            if let Some(editor) = app.active_editor() {
                let cursor_line = editor.buffer.char_to_line(editor.view.cursor);
                let cursor_row = cursor_line.saturating_sub(editor.view.scroll_row);
                let cursor_col = editor.view.column(&editor.buffer);
                let rect = crate::ui::completion::popup_rect(
                    app, r.editor_text, cursor_row, cursor_col, editor.view.scroll_col);
                if r.contains(rect, col, row) {
                    let index = row.saturating_sub(rect.y + 1) as usize;
                    if index < app.completion.items.len() {
                        app.completion.selected = index;
                        app.accept_completion();
                    }
                    return;
                }
            }
        }
        if app.active_editor().map(|e| e.is_welcome).unwrap_or(false) {
            if let Some(index) = crate::ui::welcome::hit_test(app, r.editor_text, row) {
                app.activate_welcome(index);
            }
            return;
        }
        let now = Instant::now();
        let is_double = app.last_click
            .map(|(c, rr, t)| c == col && rr == row && now.duration_since(t).as_millis() < 400)
            .unwrap_or(false);
        if let Some(editor) = app.active_editor_mut() {
            let line = editor.view.scroll_row + (row - r.editor_text.y) as usize;
            if line < editor.buffer.len_lines() {
                let col_index = (col.saturating_sub(r.editor_text.x)) as usize + editor.view.scroll_col;
                let len = editor.buffer.line_content_len(line);
                let idx = editor.buffer.line_to_char(line) + col_index.min(len);
                editor.view.set_cursor(&editor.buffer, idx, false);
                if is_double {
                    let text = editor.buffer.line_text(line);
                    let chars: Vec<char> = text.chars().collect();
                    let mut start = col_index.min(chars.len());
                    let mut end = start;
                    while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') { start -= 1; }
                    while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') { end += 1; }
                    editor.view.anchor = Some(editor.buffer.line_to_char(line) + start);
                    editor.view.cursor = editor.buffer.line_to_char(line) + end;
                }
            }
        }
        app.last_click = Some((col, row, now));
        app.focus = Focus::Editor;
        return;
    }

    // Panel.
    if r.contains(r.panel_tabs, col, row) {
        let tabs = PanelTab::all();
        let mut x = r.panel_tabs.x + 1;
        for tab in tabs {
            let w = tab.label().chars().count() as u16 + 3;
            if (x..x + w).contains(&col) { app.panel.tab = tab; app.panel.visible = true; return; }
            x += w;
        }
    }
    if r.contains(r.panel_body, col, row) {
        app.panel.visible = true;
        if app.panel.tab == PanelTab::Terminal {
            let cwd = app.cwd();
            let _ = app.terminal.ensure_session(&cwd);
            app.focus = Focus::Terminal;
        } else {
            app.focus = Focus::Panel;
        }
    }
}

fn minimap_click(app: &mut App, r: &layout::Rects, row: u16) {
    if r.minimap.height == 0 { return; }
    let Some(editor) = app.active_editor_mut() else { return; };
    let total = editor.buffer.len_lines();
    let local_row = row.saturating_sub(r.minimap.y) as usize;
    let line = crate::ui::minimap::row_to_line(local_row, total, r.minimap.height as usize);
    let half = (r.editor_text.height / 2) as usize;
    editor.view.scroll_row = line.saturating_sub(half)
        .min(total.saturating_sub(r.editor_text.height as usize));
    editor.view.cursor = editor.buffer.line_to_char(line);
    editor.view.anchor = None;
    editor.view.desired_col = 0;
}

fn char_to_byte(s: &str, char_index: usize) -> usize {
    s.char_indices().nth(char_index).map(|(i, _)| i).unwrap_or(s.len())
}
