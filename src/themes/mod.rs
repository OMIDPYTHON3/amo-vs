pub mod dark_plus;
pub mod presets;
pub mod theme;

pub use presets::{builtin, slug, BUILTIN_NAMES};
pub use theme::{SyntaxRole, Theme};

use crate::platform;
use std::path::PathBuf;

pub fn config_path() -> PathBuf {
    platform::config_dir().join("themes")
}

/// Load a theme by name. User-provided TOML in the config directory takes
/// precedence; otherwise a complete built-in palette is used.
pub fn load(name: &str) -> Theme {
    let path = config_path().join(format!("{}.toml", slug(name)));
    if let Ok(text) = std::fs::read_to_string(&path) {
        if let Ok(t) = toml::from_str::<Theme>(&text) {
            return t;
        }
    }
    builtin(name).unwrap_or_else(dark_plus::dark_plus)
}

/// Apply a theme immediately and persist the selection.
pub fn apply(app_theme: &mut Theme, config: &mut crate::services::Config, name: &str) -> bool {
    let Some(theme) = builtin(name) else { return false; };
    *app_theme = theme;
    config.theme = name.to_string();
    let _ = config.save();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_builtin_themes_have_complete_palettes() {
        for name in BUILTIN_NAMES {
            let t = builtin(name).unwrap();
            assert_eq!(t.name, name);
            // Every major surface must be a real color, not a default/reset.
            for c in [t.editor_bg, t.editor_fg, t.activity_bar_bg, t.side_bar_bg,
                      t.tab_active_bg, t.status_bar_bg, t.panel_bg, t.selection_bg,
                      t.cursor_bg, t.syntax_keyword, t.syntax_string, t.syntax_comment,
                      t.syntax_function, t.widget_bg, t.focus_border, t.minimap_bg] {
                assert!(matches!(c, ratatui::style::Color::Rgb(..)), "theme {} has incomplete palette", name);
            }
        }
    }

    #[test]
    fn themes_are_distinct() {
        let mut bgs: Vec<_> = presets::all().iter().map(|t| t.editor_bg).collect();
        bgs.sort_by_key(|c| format!("{:?}", c));
        bgs.dedup();
        assert_eq!(bgs.len(), BUILTIN_NAMES.len(), "themes should use distinct editor backgrounds");
    }
}
