use crate::app::App;
use crate::editor::Language;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use std::path::Path;
use unicode_width::UnicodeWidthStr;

const ICON_WIDTH: usize = 2;

/// VS Code Seti-inspired icon colors (Dark+). Fixed colors are used because
/// file-icon hues are intentionally distinct from syntax colors.
const RUST: Color = Color::Rgb(227, 140, 0);
const JAVASCRIPT: Color = Color::Rgb(229, 192, 123);
const TYPESCRIPT: Color = Color::Rgb(81, 154, 186);
const PYTHON: Color = Color::Rgb(101, 165, 223);
const C: Color = Color::Rgb(89, 158, 255);
const JAVA: Color = Color::Rgb(231, 111, 0);
const GO: Color = Color::Rgb(0, 173, 216);
const JSON: Color = Color::Rgb(203, 203, 65);
const HTML: Color = Color::Rgb(227, 76, 38);
const CSS: Color = Color::Rgb(86, 61, 124);
const MARKDOWN: Color = Color::Rgb(81, 154, 186);
const TOML: Color = Color::Rgb(156, 66, 33);
const SHELL: Color = Color::Rgb(137, 224, 81);
const YAML: Color = Color::Rgb(163, 177, 138);
const SQL: Color = Color::Rgb(210, 155, 90);
const CONFIG: Color = Color::Rgb(156, 165, 180);
const FOLDER: Color = Color::Rgb(192, 149, 83);
const PLAIN: Color = Color::Rgb(150, 150, 150);

pub fn icon_width() -> usize { ICON_WIDTH }

/// Language-derived glyph in the configured glyph set.
pub fn glyph(language: Language, nerd: bool) -> &'static str {
    if nerd {
        match language {
            Language::Rust => "\u{e7a8}",
            Language::JavaScript => "\u{e60c}",
            Language::TypeScript => "\u{e628}",
            Language::Python => "\u{e606}",
            Language::C => "\u{e61e}",
            Language::Cpp => "\u{e61d}",
            Language::Java => "\u{e638}",
            Language::Go => "\u{e627}",
            Language::Json => "\u{e60b}",
            Language::Html => "\u{e736}",
            Language::Css => "\u{e749}",
            Language::Markdown => "\u{e73f}",
            Language::Toml => "\u{e930}",
            Language::Shell => "\u{e795}",
            Language::Yaml => "\u{e615}",
            Language::Sql => "\u{e706}",
            Language::PlainText => "\u{f15b}",
        }
    } else {
        match language {
            Language::Rust => "R",
            Language::JavaScript => "JS",
            Language::TypeScript => "TS",
            Language::Python => "Py",
            Language::C => "C",
            Language::Cpp => "C+",
            Language::Java => "Jv",
            Language::Go => "Go",
            Language::Json => "{}",
            Language::Html => "<>",
            Language::Css => "#",
            Language::Markdown => "M",
            Language::Toml => "[]",
            Language::Shell => "$_",
            Language::Yaml => "≡",
            Language::Sql => "D",
            Language::PlainText => "·",
        }
    }
}

pub fn folder_glyph(nerd: bool, expanded: bool) -> &'static str {
    if nerd {
        if expanded { "\u{f07c}" } else { "\u{f07b}" }
    } else if expanded { "▣" } else { "▤" }
}

/// Color associated with a language icon.
pub fn language_color(language: Language) -> Color {
    match language {
        Language::Rust => RUST,
        Language::JavaScript => JAVASCRIPT,
        Language::TypeScript => TYPESCRIPT,
        Language::Python => PYTHON,
        Language::C | Language::Cpp => C,
        Language::Java => JAVA,
        Language::Go => GO,
        Language::Json => JSON,
        Language::Html => HTML,
        Language::Css => CSS,
        Language::Markdown => MARKDOWN,
        Language::Toml => TOML,
        Language::Shell => SHELL,
        Language::Yaml => YAML,
        Language::Sql => SQL,
        Language::PlainText => PLAIN,
    }
}

/// Special filenames that get a distinctive icon regardless of extension.
pub fn special_file(name: &str, nerd: bool) -> Option<(&'static str, Color)> {
    let lower = name.to_ascii_lowercase();
    let icon = match lower.as_str() {
        "cargo.toml" | "package.json" | "settings.toml" | ".env" | ".env.local"
        | "dockerfile" | "makefile" | "cmakelists.txt" | ".editorconfig" => {
            if nerd { "\u{e930}" } else { "⚙" }
        }
        ".gitignore" | ".gitattributes" | ".gitmodules" => {
            if nerd { "\u{f1d3}" } else { "⑂" }
        }
        "license" | "license.md" | "license.txt" => {
            if nerd { "\u{f02d}" } else { "©" }
        }
        "readme.md" | "readme" => {
            if nerd { "\u{e73f}" } else { "M" }
        }
        _ => return None,
    };
    Some((icon, CONFIG))
}

fn pad(glyph: &str) -> String {
    let width = UnicodeWidthStr::width(glyph);
    if width >= ICON_WIDTH { glyph.to_string() } else { format!("{}{}", glyph, " ".repeat(ICON_WIDTH - width)) }
}

/// Build a fixed-width icon span for a language (used in tabs and breadcrumbs).
pub fn icon_span(app: &App, language: Language, _reserved: usize) -> Span<'static> {
    if !app.config.render_icons {
        return Span::styled(" ".repeat(ICON_WIDTH), Style::default());
    }
    let nerd = app.config.icon_set == crate::services::IconSet::Nerd;
    Span::styled(pad(glyph(language, nerd)), Style::default().fg(language_color(language)))
}

/// Build an icon span for a path in the explorer / file browser.
pub fn icon_for_path(app: &App, path: &Path, is_dir: bool, expanded: bool) -> Span<'static> {
    if !app.config.render_icons {
        return Span::styled(" ".repeat(ICON_WIDTH), Style::default());
    }
    let nerd = app.config.icon_set == crate::services::IconSet::Nerd;
    if is_dir {
        return Span::styled(pad(folder_glyph(nerd, expanded)), Style::default().fg(FOLDER));
    }
    let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    if let Some((icon, color)) = special_file(&name, nerd) {
        return Span::styled(pad(icon), Style::default().fg(color));
    }
    let language = Language::detect(path);
    Span::styled(pad(glyph(language, nerd)), Style::default().fg(language_color(language)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::Config;
    use crate::themes::Theme;

    fn app() -> App { App::new(Config::default(), Theme::default(), None, 120, 32) }

    #[test]
    fn simple_glyphs_are_single_column() {
        for lang in [Language::Rust, Language::JavaScript, Language::TypeScript,
                     Language::Python, Language::C, Language::Cpp, Language::Java,
                     Language::Go, Language::Json, Language::Html, Language::Css,
                     Language::Markdown, Language::Toml, Language::Shell,
                     Language::Yaml, Language::Sql, Language::PlainText] {
            let g = glyph(lang, false);
            assert!(UnicodeWidthStr::width(g) <= 2, "glyph too wide for {:?}", lang);
            assert!(!g.is_empty());
        }
    }

    #[test]
    fn nerd_glyphs_are_distinct() {
        let mut glyphs: Vec<&str> = Vec::new();
        for lang in [Language::Rust, Language::JavaScript, Language::Python, Language::Go] {
            let g = glyph(lang, true);
            assert!(!glyphs.contains(&g));
            glyphs.push(g);
        }
    }

    #[test]
    fn special_files_get_icons() {
        assert!(special_file("Cargo.toml", false).is_some());
        assert!(special_file(".gitignore", false).is_some());
        assert!(special_file("random.bin", false).is_none());
    }

    #[test]
    fn icon_spans_have_fixed_width() {
        let a = app();
        for lang in [Language::Rust, Language::Python, Language::Json] {
            let span = icon_span(&a, lang, 0);
            assert_eq!(span.content.chars().count(), ICON_WIDTH);
        }
        let folder = icon_for_path(&a, Path::new("src"), true, false);
        assert_eq!(folder.content.chars().count(), ICON_WIDTH);
    }

    #[test]
    fn icons_can_be_disabled() {
        let mut a = app();
        a.config.render_icons = false;
        let span = icon_span(&a, Language::Rust, 0);
        assert_eq!(span.content.chars().count(), ICON_WIDTH);
    }
}
