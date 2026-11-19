use super::theme::Theme;
use ratatui::style::Color;

fn hex(s: &str) -> Color {
    let s = s.trim_start_matches('#');
    let r = u8::from_str_radix(&s[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&s[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&s[4..6], 16).unwrap_or(0);
    Color::Rgb(r, g, b)
}

/// Visual Studio Code Dark+ (Default Dark+) color palette.
pub fn dark_plus() -> Theme {
    Theme {
        name: "Dark+ (default dark)".into(),

        bg: hex("#1E1E1E"),
        fg: hex("#D4D4D4"),
        editor_bg: hex("#1E1E1E"),
        editor_fg: hex("#D4D4D4"),

        activity_bar_bg: hex("#333333"),
        activity_bar_fg: hex("#858585"),
        activity_bar_active_fg: hex("#FFFFFF"),
        activity_bar_border: hex("#2B2B2B"),
        activity_bar_badge: hex("#007ACC"),

        side_bar_bg: hex("#252526"),
        side_bar_fg: hex("#CCCCCC"),
        side_bar_title: hex("#BBBBBB"),
        side_bar_border: hex("#333333"),
        side_bar_active_bg: hex("#37373D"),
        side_bar_hover_bg: hex("#2A2D2E"),
        side_bar_section_header_bg: hex("#252526"),
        side_bar_indent_guide: hex("#404040"),

        menu_bg: hex("#3C3C3C"),
        menu_fg: hex("#CCCCCC"),
        menu_active_bg: hex("#007ACC"),
        menu_active_fg: hex("#FFFFFF"),

        tab_active_bg: hex("#1E1E1E"),
        tab_inactive_bg: hex("#2D2D2D"),
        tab_active_fg: hex("#FFFFFF"),
        tab_inactive_fg: hex("#969696"),
        tab_border: hex("#2B2B2B"),
        tab_active_border: hex("#007ACC"),
        tab_hover_bg: hex("#2D2D2D"),
        tab_dirty: hex("#FFFFFF"),

        status_bar_bg: hex("#007ACC"),
        status_bar_fg: hex("#FFFFFF"),
        status_bar_accent_bg: hex("#16825D"),
        status_bar_accent_fg: hex("#FFFFFF"),
        status_bar_warning_bg: hex("#BF8803"),
        status_bar_error_bg: hex("#E51400"),

        panel_bg: hex("#1E1E1E"),
        panel_fg: hex("#CCCCCC"),
        panel_border: hex("#414141"),
        panel_active: hex("#E7E7E7"),
        panel_inactive: hex("#858585"),

        line_number_fg: hex("#858585"),
        line_number_active_fg: hex("#C6C6C6"),
        current_line_bg: hex("#282828"),
        selection_bg: hex("#264F78"),
        cursor_bg: hex("#AEAFAD"),
        bracket_match_bg: hex("#3A3D41"),
        bracket_match_border: hex("#A6A6A6"),
        indent_guide: hex("#404040"),
        whitespace: hex("#3C3C3C"),

        focus_border: hex("#007FD4"),
        widget_bg: hex("#252526"),
        widget_border: hex("#454545"),
        input_bg: hex("#3C3C3C"),
        input_border: hex("#007FD4"),
        list_active_bg: hex("#04395E"),
        list_inactive_bg: hex("#2D2D30"),
        scrollbar_thumb: hex("#797979"),
        scrollbar_track: Color::Reset,

        git_added: hex("#81B88B"),
        git_modified: hex("#E2C08D"),
        git_deleted: hex("#C74E39"),
        git_conflict: hex("#F14C4C"),
        git_untracked: hex("#81B88B"),
        git_ignored: hex("#616161"),

        error: hex("#F14C4C"),
        warning: hex("#CCA700"),
        info: hex("#3794FF"),

        syntax_keyword: hex("#569CD6"),
        syntax_control: hex("#C586C0"),
        syntax_string: hex("#CE9178"),
        syntax_number: hex("#B5CEA8"),
        syntax_comment: hex("#6A9955"),
        syntax_function: hex("#DCDCAA"),
        syntax_type: hex("#4EC9B0"),
        syntax_variable: hex("#9CDCFE"),
        syntax_property: hex("#9CDCFE"),
        syntax_constant: hex("#569CD6"),
        syntax_operator: hex("#D4D4D4"),
        syntax_punctuation: hex("#D4D4D4"),
        syntax_attribute: hex("#9CDCFE"),
        syntax_tag: hex("#569CD6"),
        syntax_tag_attr: hex("#9CDCFE"),
        syntax_macro: hex("#DCDCAA"),
        syntax_string_escape: hex("#D7BA7D"),
        syntax_enum_member: hex("#9CDCFE"),
        syntax_type_parameter: hex("#4EC9B0"),
        syntax_regexp: hex("#D16969"),
        minimap_bg: hex("#1E1E1E"),
        minimap_viewport_bg: hex("#2D2F30"),
        minimap_slider: hex("#696969"),
        tree_guide: hex("#404040"),
    }
}

/// A high-contrast light theme shipped for users who prefer it.
pub fn light_plus() -> Theme {
    let mut t = dark_plus();
    t.name = "Light+ (default light)".into();
    t.bg = hex("#FFFFFF");
    t.fg = hex("#000000");
    t.editor_bg = hex("#FFFFFF");
    t.editor_fg = hex("#000000");
    t.activity_bar_bg = hex("#2C2C2C");
    t.side_bar_bg = hex("#F3F3F3");
    t.side_bar_fg = hex("#424242");
    t.side_bar_title = hex("#424242");
    t.side_bar_border = hex("#E5E5E5");
    t.side_bar_active_bg = hex("#E4E6F1");
    t.side_bar_hover_bg = hex("#E8E8E8");
    t.menu_bg = hex("#007ACC");
    t.menu_fg = hex("#FFFFFF");
    t.tab_active_bg = hex("#FFFFFF");
    t.tab_inactive_bg = hex("#ECECEC");
    t.tab_active_fg = hex("#000000");
    t.tab_inactive_fg = hex("#666666");
    t.tab_border = hex("#E5E5E5");
    t.status_bar_bg = hex("#007ACC");
    t.panel_bg = hex("#FFFFFF");
    t.panel_fg = hex("#424242");
    t.panel_border = hex("#D4D4D4");
    t.line_number_fg = hex("#237893");
    t.line_number_active_fg = hex("#000000");
    t.current_line_bg = hex("#F0F0F0");
    t.selection_bg = hex("#ADD6FF");
    t.cursor_bg = hex("#000000");
    t.widget_bg = hex("#F3F3F3");
    t.widget_border = hex("#C6C6C6");
    t.input_bg = hex("#FFFFFF");
    t.list_active_bg = hex("#CDE6FF");
    t.list_inactive_bg = hex("#E4E6F1");
    t.syntax_keyword = hex("#0000FF");
    t.syntax_control = hex("#AF00DB");
    t.syntax_string = hex("#A31515");
    t.syntax_number = hex("#098658");
    t.syntax_comment = hex("#008000");
    t.syntax_function = hex("#795E26");
    t.syntax_type = hex("#267F99");
    t.syntax_variable = hex("#001080");
    t.syntax_property = hex("#001080");
    t.syntax_constant = hex("#0000FF");
    t.syntax_tag = hex("#800000");
    t.syntax_tag_attr = hex("#E50000");
    t.syntax_macro = hex("#795E26");
    t.syntax_string_escape = hex("#D7BA7D");
    t.syntax_enum_member = hex("#001080");
    t.syntax_type_parameter = hex("#267F99");
    t.syntax_regexp = hex("#A31515");
    t.minimap_bg = hex("#FFFFFF");
    t.minimap_viewport_bg = hex("#E8E8E8");
    t.minimap_slider = hex("#B0B0B0");
    t.tree_guide = hex("#D0D0D0");
    t
}
