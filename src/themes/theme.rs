use ratatui::style::{Color, Modifier, Style};
use serde::{Deserialize, Serialize};

/// Semantic syntax roles shared between the highlighter and the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SyntaxRole {
    Plain,
    Keyword,
    ControlKeyword,
    String,
    Number,
    Comment,
    Function,
    Type,
    Variable,
    Property,
    Constant,
    Operator,
    Punctuation,
    Attribute,
    Tag,
    TagAttribute,
    Macro,
    StringEscape,
    EnumMember,
    TypeParameter,
    Regexp,
}

/// Every color used by the editor UI. Defaults mirror VS Code Dark+.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,

    pub bg: Color,
    pub fg: Color,
    pub editor_bg: Color,
    pub editor_fg: Color,

    pub activity_bar_bg: Color,
    pub activity_bar_fg: Color,
    pub activity_bar_active_fg: Color,
    pub activity_bar_border: Color,
    pub activity_bar_badge: Color,

    pub side_bar_bg: Color,
    pub side_bar_fg: Color,
    pub side_bar_title: Color,
    pub side_bar_border: Color,
    pub side_bar_active_bg: Color,
    pub side_bar_hover_bg: Color,
    pub side_bar_section_header_bg: Color,
    pub side_bar_indent_guide: Color,

    pub menu_bg: Color,
    pub menu_fg: Color,
    pub menu_active_bg: Color,
    pub menu_active_fg: Color,

    pub tab_active_bg: Color,
    pub tab_inactive_bg: Color,
    pub tab_active_fg: Color,
    pub tab_inactive_fg: Color,
    pub tab_border: Color,
    pub tab_active_border: Color,
    pub tab_hover_bg: Color,
    pub tab_dirty: Color,

    pub status_bar_bg: Color,
    pub status_bar_fg: Color,
    pub status_bar_accent_bg: Color,
    pub status_bar_accent_fg: Color,
    pub status_bar_warning_bg: Color,
    pub status_bar_error_bg: Color,

    pub panel_bg: Color,
    pub panel_fg: Color,
    pub panel_border: Color,
    pub panel_active: Color,
    pub panel_inactive: Color,

    pub line_number_fg: Color,
    pub line_number_active_fg: Color,
    pub current_line_bg: Color,
    pub selection_bg: Color,
    pub cursor_bg: Color,
    pub bracket_match_bg: Color,
    pub bracket_match_border: Color,
    pub indent_guide: Color,
    pub whitespace: Color,

    pub focus_border: Color,
    pub widget_bg: Color,
    pub widget_border: Color,
    pub input_bg: Color,
    pub input_border: Color,
    pub list_active_bg: Color,
    pub list_inactive_bg: Color,
    pub scrollbar_thumb: Color,
    pub scrollbar_track: Color,

    pub git_added: Color,
    pub git_modified: Color,
    pub git_deleted: Color,
    pub git_conflict: Color,
    pub git_untracked: Color,
    pub git_ignored: Color,

    pub error: Color,
    pub warning: Color,
    pub info: Color,

    pub syntax_keyword: Color,
    pub syntax_control: Color,
    pub syntax_string: Color,
    pub syntax_number: Color,
    pub syntax_comment: Color,
    pub syntax_function: Color,
    pub syntax_type: Color,
    pub syntax_variable: Color,
    pub syntax_property: Color,
    pub syntax_constant: Color,
    pub syntax_operator: Color,
    pub syntax_punctuation: Color,
    pub syntax_attribute: Color,
    pub syntax_tag: Color,
    pub syntax_tag_attr: Color,

    // Extended roles introduced after the first release. Per-field defaults
    // keep older theme TOML files compatible.
    #[serde(default = "default_macro")]
    pub syntax_macro: Color,
    #[serde(default = "default_string_escape")]
    pub syntax_string_escape: Color,
    #[serde(default = "default_enum_member")]
    pub syntax_enum_member: Color,
    #[serde(default = "default_type_parameter")]
    pub syntax_type_parameter: Color,
    #[serde(default = "default_regexp")]
    pub syntax_regexp: Color,

    #[serde(default = "default_minimap_bg")]
    pub minimap_bg: Color,
    #[serde(default = "default_minimap_viewport")]
    pub minimap_viewport_bg: Color,
    #[serde(default = "default_minimap_slider")]
    pub minimap_slider: Color,
    #[serde(default = "default_guide")]
    pub tree_guide: Color,
}

fn default_macro() -> Color { Color::Rgb(220, 220, 170) }
fn default_string_escape() -> Color { Color::Rgb(215, 186, 125) }
fn default_enum_member() -> Color { Color::Rgb(156, 220, 254) }
fn default_type_parameter() -> Color { Color::Rgb(78, 201, 176) }
fn default_regexp() -> Color { Color::Rgb(209, 105, 105) }
fn default_minimap_bg() -> Color { Color::Rgb(30, 30, 30) }
fn default_minimap_viewport() -> Color { Color::Rgb(45, 47, 49) }
fn default_minimap_slider() -> Color { Color::Rgb(105, 105, 105) }
fn default_guide() -> Color { Color::Rgb(64, 64, 64) }

impl Theme {
    pub fn style(&self, role: SyntaxRole) -> Style {
        let c = match role {
            SyntaxRole::Plain => self.editor_fg,
            SyntaxRole::Keyword => self.syntax_keyword,
            SyntaxRole::ControlKeyword => self.syntax_control,
            SyntaxRole::String => self.syntax_string,
            SyntaxRole::Number => self.syntax_number,
            SyntaxRole::Comment => self.syntax_comment,
            SyntaxRole::Function => self.syntax_function,
            SyntaxRole::Type => self.syntax_type,
            SyntaxRole::Variable => self.syntax_variable,
            SyntaxRole::Property => self.syntax_property,
            SyntaxRole::Constant => self.syntax_constant,
            SyntaxRole::Operator => self.syntax_operator,
            SyntaxRole::Punctuation => self.syntax_punctuation,
            SyntaxRole::Attribute => self.syntax_attribute,
            SyntaxRole::Tag => self.syntax_tag,
            SyntaxRole::TagAttribute => self.syntax_tag_attr,
            SyntaxRole::Macro => self.syntax_macro,
            SyntaxRole::StringEscape => self.syntax_string_escape,
            SyntaxRole::EnumMember => self.syntax_enum_member,
            SyntaxRole::TypeParameter => self.syntax_type_parameter,
            SyntaxRole::Regexp => self.syntax_regexp,
        };
        Style::default().fg(c)
    }

    pub fn git_style(&self, state: crate::workspace::GitState) -> Style {
        let c = match state {
            crate::workspace::GitState::Modified => self.git_modified,
            crate::workspace::GitState::Added | crate::workspace::GitState::Untracked => self.git_added,
            crate::workspace::GitState::Deleted => self.git_deleted,
            crate::workspace::GitState::Conflict => self.git_conflict,
            crate::workspace::GitState::Ignored => self.git_ignored,
            crate::workspace::GitState::Clean => self.side_bar_fg,
        };
        Style::default().fg(c)
    }

    pub fn modifier(italic: bool, bold: bool) -> Modifier {
        let mut m = Modifier::empty();
        if italic {
            m |= Modifier::ITALIC;
        }
        if bold {
            m |= Modifier::BOLD;
        }
        m
    }
}

impl Default for Theme {
    fn default() -> Self {
        crate::themes::dark_plus::dark_plus()
    }
}
