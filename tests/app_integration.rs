use amo_vs::app::events;
use amo_vs::app::{Action, App, ConfirmStyle, Focus, Overlay};
use amo_vs::services::IconSet;
use amo_vs::platform::terminal_restore_sequence;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use amo_vs::editor::Language;
use amo_vs::services::{fuzzy, Config};
use amo_vs::themes::Theme;
use amo_vs::workspace::search;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_workspace() -> PathBuf {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "amo-vs-integration-{}-{}",
        std::process::id(),
        id
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("README.md"), "# Demo\nhello world\n").unwrap();
    fs::write(dir.join("src/main.rs"), "fn main() {\n    println!(\"hello rust\");\n}\n").unwrap();
    dir
}

fn new_app(root: &Path) -> App {
    App::new(Config::default(), Theme::default(), Some(root.to_path_buf()), 120, 32)
}

#[test]
fn opens_file_edits_and_saves() {
    let root = temp_workspace();
    let file = root.join("src/main.rs");
    let mut app = new_app(&root);

    app.open_path(&file).unwrap();
    assert_eq!(app.active_editor().unwrap().name(), "main.rs");
    assert_eq!(app.active_editor().unwrap().buffer.language, Language::Rust);

    let editor = app.active_editor_mut().unwrap();
    editor.view.set_cursor(&editor.buffer, editor.buffer.len_chars(), false);
    editor.insert_text("// edited\n", &Config::default());
    assert!(editor.dirty());
    app.save();

    let saved = fs::read_to_string(&file).unwrap();
    assert!(saved.contains("// edited"));
    assert!(!app.active_editor().unwrap().dirty());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_exposes_file_tree_and_search() {
    let root = temp_workspace();
    let mut app = new_app(&root);
    app.workspace.as_mut().unwrap().toggle(&root.join("src"));
    let workspace = app.workspace.as_ref().unwrap();
    let entries = workspace.flatten();
    let names: Vec<String> = entries.iter().map(|e| e.name.clone()).collect();
    assert!(names.contains(&"src".to_string()));
    assert!(names.contains(&"main.rs".to_string()));
    assert!(names.contains(&"README.md".to_string()));

    let results = search::search(&root, "hello", &Config::default(), false, false, 50);
    assert_eq!(results.len(), 2);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn git_status_detects_modification() {
    if Command::new("git").arg("--version").output().is_err() {
        return;
    }
    let root = temp_workspace();
    let _ = Command::new("git").args(["init", "-q"]).current_dir(&root).status();
    let _ = Command::new("git").args(["add", "."]).current_dir(&root).status();
    let _ = Command::new("git")
        .args(["-c", "user.name=Test", "-c", "user.email=test@example.com", "commit", "-qm", "init"])
        .current_dir(&root)
        .status();
    fs::write(root.join("README.md"), "# Demo\nchanged\n").unwrap();

    let app = new_app(&root);
    assert!(app.workspace.as_ref().unwrap().git.branch.is_some());
    assert!(app.git_changes() >= 1);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn config_serializes_to_toml() {
    let config = Config::default();
    let text = toml::to_string(&config).unwrap();
    let parsed: Config = toml::from_str(&text).unwrap();
    assert_eq!(parsed.tab_size, 4);
    assert_eq!(parsed.theme, config.theme);
    assert!(parsed.insert_spaces);
}

#[test]
fn fuzzy_ranking_prefers_prefix_and_boundaries() {
    let items = ["Go to File", "Git: Commit", "Find File"];
    let ranked = fuzzy::fuzzy_filter("gofile", &items, 10);
    assert!(!ranked.is_empty());
    assert_eq!(ranked[0].0, 0);
}

#[test]
fn startup_file_becomes_the_active_tab() {
    let root = temp_workspace();
    let file = root.join("src/main.rs");
    let mut app = new_app(&root);
    assert_eq!(app.editors.len(), 1); // welcome buffer
    app.open_startup(&file);
    assert_eq!(app.editors.len(), 1); // welcome replaced by the file
    assert_eq!(app.active_editor().unwrap().name(), "main.rs");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn shipped_theme_is_valid_toml() {
    let source = include_str!("../themes/dark_plus.toml");
    let theme: Theme = toml::from_str(source).unwrap();
    assert_eq!(theme.name, "Dark+ (default dark)");
    assert_eq!(theme.editor_bg, ratatui::style::Color::Rgb(30, 30, 30));
}

fn dirty_app(root: &Path) -> App {
    let mut app = new_app(root);
    app.open_path(&root.join("src/main.rs")).unwrap();
    let editor = app.active_editor_mut().unwrap();
    editor.view.set_cursor(&editor.buffer, editor.buffer.len_chars(), false);
    editor.insert_text("// dirty\n", &Config::default());
    assert!(app.active_editor().unwrap().dirty());
    app
}

#[test]
fn exit_without_unsaved_changes_is_immediate() {
    let root = temp_workspace();
    let mut app = new_app(&root);
    assert!(!app.editors.iter().any(|e| e.dirty()));
    app.execute(Action::Quit);
    assert!(app.quit);
    assert!(app.confirm.is_none());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn exit_with_unsaved_changes_shows_three_option_dialog() {
    let root = temp_workspace();
    let mut app = dirty_app(&root);
    app.execute(Action::Quit);
    assert!(!app.quit);
    let confirm = app.confirm.as_ref().unwrap();
    assert_eq!(confirm.style, ConfirmStyle::Exit);
    assert_eq!(confirm.option_count(), 3);
    assert_eq!(app.overlay, Some(Overlay::Confirm));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn save_all_and_exit_persists_files_and_quits() {
    let root = temp_workspace();
    let file = root.join("src/main.rs");
    let mut app = dirty_app(&root);
    app.execute(Action::Quit);
    app.execute(Action::SaveAllAndExit);
    assert!(app.quit);
    assert!(app.confirm.is_none());
    assert!(fs::read_to_string(&file).unwrap().contains("// dirty"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn exit_without_saving_discards_changes() {
    let root = temp_workspace();
    let file = root.join("src/main.rs");
    let mut app = dirty_app(&root);
    app.execute(Action::Quit);
    app.execute(Action::ExitWithoutSaving);
    assert!(app.quit);
    assert!(!fs::read_to_string(&file).unwrap().contains("// dirty"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cancel_exit_keeps_application_running() {
    let root = temp_workspace();
    let mut app = dirty_app(&root);
    app.execute(Action::Quit);
    app.execute(Action::CancelExit);
    assert!(!app.quit);
    assert!(app.confirm.is_none());
    assert!(app.active_editor().unwrap().dirty());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn ctrl_esc_works_from_every_focus_and_dialog() {
    let root = temp_workspace();
    let clean = new_app(&root);
    for focus in [Focus::Editor, Focus::Explorer, Focus::Terminal, Focus::Panel] {
        let mut app = new_app(&root);
        app.focus = focus;
        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::CONTROL);
        events::handle(&mut app, Event::Key(key));
        assert!(app.quit, "Ctrl+Esc failed from {:?}", focus);
    }
    // Even while a modal is open, the global shortcut is honoured.
    let mut app = clean;
    app.overlay = Some(Overlay::Help);
    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::CONTROL);
    events::handle(&mut app, Event::Key(key));
    assert!(app.quit);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn plain_esc_is_portable_exit_fallback_outside_terminal() {
    let root = temp_workspace();
    let mut app = new_app(&root);
    app.focus = Focus::Editor;
    events::handle(&mut app, Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty())));
    assert!(app.quit);

    let mut app = new_app(&root);
    app.focus = Focus::Terminal;
    events::handle(&mut app, Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty())));
    assert!(!app.quit, "Esc must reach shell programs in terminal focus");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn ctrl_esc_from_dirty_editor_shows_exit_dialog() {
    let root = temp_workspace();
    let mut app = dirty_app(&root);
    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::CONTROL);
    events::handle(&mut app, Event::Key(key));
    assert_eq!(app.overlay, Some(Overlay::Confirm));
    assert_eq!(app.confirm.as_ref().unwrap().style, ConfirmStyle::Exit);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn exit_dialog_keyboard_shortcuts() {
    let root = temp_workspace();
    let file = root.join("src/main.rs");
    let mut app = dirty_app(&root);
    app.execute(Action::Quit);
    // '2' = Exit Without Saving
    events::handle(&mut app, Event::Key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::empty())));
    assert!(app.quit);
    assert!(!fs::read_to_string(&file).unwrap().contains("// dirty"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn terminal_restore_sequence_disables_every_mode() {
    let seq = terminal_restore_sequence(true);
    let text = String::from_utf8(seq).unwrap();
    assert!(text.contains("\x1b[?1049l"), "must leave alternate screen");
    assert!(text.contains("\x1b[?25h"), "must show cursor");
    assert!(text.contains("\x1b[?1000l"), "must disable mouse capture");
    assert!(text.contains("\x1b[>4;0m"), "must pop keyboard flags");
}

#[test]
fn app_shutdown_closes_terminal_sessions() {
    let root = temp_workspace();
    let mut app = new_app(&root);
    // No sessions spawned in tests; shutdown must still be safe/idempotent.
    app.shutdown();
    assert!(app.terminal.sessions.is_empty());
    app.shutdown();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn welcome_page_is_shown_and_navigable() {
    let root = temp_workspace();
    let mut app = new_app(&root);
    assert!(app.active_editor().unwrap().is_welcome);
    let actions = app.welcome_actions();
    assert!(actions.len() >= 3);
    app.welcome_selected = 0;
    app.activate_welcome(0);
    assert!(app.active_editor().map(|e| !e.is_welcome).unwrap_or(true));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn recent_projects_are_persisted() {
    let root = temp_workspace();
    let mut app = new_app(&root);
    app.open_workspace(root.clone()).unwrap();
    assert!(!app.recent_projects.is_empty());
    assert_eq!(app.recent_projects[0], root);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn context_menu_offers_file_operations() {
    let root = temp_workspace();
    let file = root.join("README.md");
    let mut app = new_app(&root);
    app.open_context_menu(file.clone(), false, 10, 10);
    assert_eq!(app.overlay, Some(Overlay::ContextMenu));
    let menu = app.context_menu.as_ref().unwrap();
    let labels: Vec<&str> = menu.actions.iter().map(|a| a.label()).collect();
    assert!(labels.contains(&"Open"));
    assert!(labels.contains(&"Rename"));
    assert!(labels.contains(&"Delete"));
    assert!(labels.contains(&"Copy Path"));
    // Copy Path is safe to execute in a test.
    app.context_menu.as_mut().unwrap().selected = labels.iter().position(|l| *l == "Copy Path").unwrap();
    app.execute_context_action();
    assert!(app.context_menu.is_none());
    assert_eq!(app.clipboard.get(), file.to_string_lossy());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn keybindings_are_configurable() {
    use amo_vs::services::shortcut_matches;
    use crossterm::event::KeyModifiers;
    assert!(shortcut_matches("ctrl+esc", KeyCode::Esc, KeyModifiers::CONTROL));
    assert!(shortcut_matches("ctrl+shift+p", KeyCode::Char('P'), KeyModifiers::CONTROL | KeyModifiers::SHIFT));
    assert!(!shortcut_matches("ctrl+esc", KeyCode::Esc, KeyModifiers::empty()));
    let mut config = Config::default();
    config.keybindings.exit = "alt+f4".into();
    assert!(config.keybindings.matches("alt+f4", KeyCode::F(4), KeyModifiers::ALT));
}

#[test]
fn fallback_highlighter_marks_language_specific_tokens() {
    use amo_vs::editor::Buffer;
    use amo_vs::themes::SyntaxRole;
    let toml = Buffer::from_text("[package]\nname = \"amo\"\n", Language::Toml);
    let mut hl = amo_vs::editor::Highlighter::new();
    let spans = hl.spans(&toml, toml.revision()).to_vec();
    assert!(spans[0].iter().any(|s| s.2 == SyntaxRole::Type));
    assert!(spans[1].iter().any(|s| s.2 == SyntaxRole::Property));

    let md = Buffer::from_text("# Heading\ntext\n", Language::Markdown);
    let spans = hl.spans(&md, md.revision()).to_vec();
    assert!(spans[0].iter().any(|s| s.2 == SyntaxRole::Keyword));

    let json = Buffer::from_text("{\"key\": \"value\"}", Language::Json);
    let spans = hl.spans(&json, json.revision()).to_vec();
    assert!(spans[0].iter().any(|s| s.2 == SyntaxRole::Property));
}

#[test]
fn exit_dialog_default_is_save_all() {
    let root = temp_workspace();
    let mut app = dirty_app(&root);
    app.execute(Action::Quit);
    assert_eq!(app.confirm.as_ref().unwrap().selected, 0);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn highlighter_produces_spans_and_diagnostics() {
    let root = temp_workspace();
    let mut app = new_app(&root);
    app.open_path(&root.join("src/main.rs")).unwrap();
    let editor = app.active_editor_mut().unwrap();
    let spans = editor.highlighter.spans(&editor.buffer, editor.buffer.revision()).to_vec();
    assert_eq!(spans.len(), 4);
    assert!(spans[0].iter().any(|s| s.2 == amo_vs::themes::SyntaxRole::Keyword));
    let _ = fs::remove_dir_all(root);
}

// ---------------------------------------------------------------------------
// 14. Intelligent completion
// ---------------------------------------------------------------------------

#[test]
fn completion_popup_updates_and_accepts() {
    let root = temp_workspace();
    let mut app = new_app(&root);
    app.open_path(&root.join("src/main.rs")).unwrap();
    let config = Config::default();
    {
        let editor = app.active_editor_mut().unwrap();
        editor.view.set_cursor(&editor.buffer, editor.buffer.len_chars(), false);
        editor.insert_text("\nfn helper", &config);
    }
    app.update_completion();
    assert!(app.completion.visible);
    assert!(app.completion.items.iter().any(|i| i.label == "helper"));
    app.accept_completion();
    assert!(!app.completion.visible);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn completion_escape_dismisses_without_exit() {
    let root = temp_workspace();
    let mut app = new_app(&root);
    app.open_path(&root.join("src/main.rs")).unwrap();
    for c in "fn hel".chars() {
        events::handle(&mut app, Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty())));
    }
    assert!(app.completion.visible);
    events::handle(&mut app, Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty())));
    assert!(!app.completion.visible);
    assert!(!app.quit, "Esc must dismiss completion, not exit");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn global_exit_beats_autocomplete_popup() {
    let root = temp_workspace();
    let mut app = new_app(&root);
    app.completion.visible = true;
    events::handle(&mut app, Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::CONTROL)));
    assert!(app.quit, "Ctrl+Esc must exit even with completion visible");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn completion_works_across_languages() {
    use amo_vs::editor::{CompletionEngine, CompletionState, Language};
    let cases = [
        ("def calculate(self):\n    return 1\nca", Language::Python),
        ("function calculate() {}\nca", Language::JavaScript),
        ("class Calculator {}\nca", Language::Java),
        ("func calculate() {}\nca", Language::Go),
        ("int calculate() { return 1; }\nca", Language::C),
    ];
    for (text, language) in cases {
        let buffer = amo_vs::editor::Buffer::from_text(text, language);
        let mut engine = CompletionEngine::new();
        let state: CompletionState = engine.compute(
            &buffer, buffer.len_chars(), None, &Config::default(),
            amo_vs::editor::CompletionContext::Code,
        ).unwrap_or_default();
        assert!(state.visible, "no suggestions for {:?}", language);
        assert!(state.items.iter().any(|i| i.label.starts_with("ca")), "no relevant item for {:?}", language);
    }
}

#[test]
fn completion_is_configurable() {
    let mut config = Config::default();
    assert!(config.autocomplete);
    config.autocomplete = false;
    config.autocomplete_max_items = 3;
    config.autocomplete_workspace = false;
    config.icon_set = IconSet::Nerd;
    let text = toml::to_string(&config).unwrap();
    let parsed: Config = toml::from_str(&text).unwrap();
    assert!(!parsed.autocomplete);
    assert_eq!(parsed.autocomplete_max_items, 3);
    assert!(!parsed.autocomplete_workspace);
    assert_eq!(parsed.icon_set, IconSet::Nerd);
}

// ---------------------------------------------------------------------------
// 15. Themes
// ---------------------------------------------------------------------------

#[test]
fn all_builtin_themes_exist_as_toml_files() {
    for name in amo_vs::themes::BUILTIN_NAMES {
        let slug = amo_vs::themes::slug(name);
        let source = fs::read_to_string(format!("themes/{}.toml", slug))
            .unwrap_or_else(|_| panic!("missing theme {}", slug));
        let from_file: Theme = toml::from_str(&source).unwrap();
        let from_code = amo_vs::themes::builtin(name).unwrap();
        assert_eq!(from_file.editor_bg, from_code.editor_bg, "theme {} differs", name);
        assert_eq!(from_file.syntax_keyword, from_code.syntax_keyword, "theme {} colors differ", name);
        assert_eq!(from_file.status_bar_bg, from_code.status_bar_bg, "theme {} colors differ", name);
    }
}

#[test]
fn theme_switch_applies_immediately() {
    let root = temp_workspace();
    let mut app = new_app(&root);
    let original = app.theme.editor_bg;
    assert!(amo_vs::themes::apply(&mut app.theme, &mut app.config, "Dracula"));
    assert_ne!(app.theme.editor_bg, original);
    assert_eq!(app.theme.name, "Dracula");
    assert_eq!(app.config.theme, "Dracula");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn theme_picker_previews_then_cancels() {
    let root = temp_workspace();
    let mut app = new_app(&root);
    let original = app.theme.clone();
    app.open_theme_picker();
    assert_eq!(app.overlay, Some(Overlay::ThemePicker));
    app.theme_picker_move(1);
    assert_ne!(app.theme.editor_bg, original.editor_bg, "preview should change theme");
    app.theme_picker_cancel();
    assert_eq!(app.theme.editor_bg, original.editor_bg);
    assert_eq!(app.overlay, None);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn theme_picker_accept_persists_selection() {
    let root = temp_workspace();
    let mut app = new_app(&root);
    app.open_theme_picker();
    app.theme_picker.as_mut().unwrap().query = "nord".into();
    app.update_theme_picker();
    app.theme_picker_accept();
    assert_eq!(app.config.theme, "Nord");
    assert_eq!(app.theme.name, "Nord");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn global_exit_works_from_theme_picker() {
    let root = temp_workspace();
    let mut app = new_app(&root);
    app.open_theme_picker();
    events::handle(&mut app, Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::CONTROL)));
    assert!(app.quit);
    let _ = fs::remove_dir_all(root);
}

// ---------------------------------------------------------------------------
// 18. Global Ctrl+Esc / exit state restoration
// ---------------------------------------------------------------------------

#[test]
fn cancel_exit_restores_previous_ui_state() {
    let root = temp_workspace();
    let mut app = dirty_app(&root);
    app.focus = Focus::Explorer;
    app.overlay = Some(Overlay::QuickOpen);
    app.palette.query = "main".into();
    app.explorer_selected = 3;
    let cursor = app.active_editor().unwrap().view.cursor;
    events::handle(&mut app, Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::CONTROL)));
    assert_eq!(app.overlay, Some(Overlay::Confirm));
    app.execute(Action::CancelExit);
    assert_eq!(app.overlay, Some(Overlay::QuickOpen));
    assert_eq!(app.focus, Focus::Explorer);
    assert_eq!(app.explorer_selected, 3);
    assert_eq!(app.palette.query, "main");
    assert_eq!(app.active_editor().unwrap().view.cursor, cursor);
    assert!(app.active_editor().unwrap().dirty());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn repeated_exit_shortcut_does_not_queue_multiple_exits() {
    let root = temp_workspace();
    let mut app = dirty_app(&root);
    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::CONTROL);
    events::handle(&mut app, Event::Key(key));
    events::handle(&mut app, Event::Key(key));
    events::handle(&mut app, Event::Key(key));
    assert!(app.exit_requested);
    assert_eq!(app.overlay, Some(Overlay::Confirm));
    app.execute(Action::ExitWithoutSaving);
    assert!(app.quit);
    app.execute(Action::Quit);
    assert!(app.quit);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn global_exit_works_from_search_and_dialog_overlays() {
    let root = temp_workspace();
    for overlay in [Overlay::GlobalSearch, Overlay::Find, Overlay::FileBrowser, Overlay::Help] {
        let mut app = new_app(&root);
        app.overlay = Some(overlay);
        events::handle(&mut app, Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::CONTROL)));
        assert!(app.quit, "Ctrl+Esc failed from {:?}", overlay);
    }
    let _ = fs::remove_dir_all(root);
}
