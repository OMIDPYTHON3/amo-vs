use crate::app::App;
use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy)]
pub struct Rects {
    pub menu: Rect,
    pub activity: Rect,
    pub sidebar: Rect,
    pub main: Rect,
    pub tabs: Rect,
    pub breadcrumbs: Rect,
    pub editor: Rect,
    pub gutter: Rect,
    pub editor_text: Rect,
    pub minimap: Rect,
    pub panel: Rect,
    pub panel_tabs: Rect,
    pub panel_body: Rect,
    pub status: Rect,
}

fn empty() -> Rect { Rect { x: 0, y: 0, width: 0, height: 0 } }

/// The sidebar is collapsed automatically when showing it would leave too
/// little room for the editor. The user's preference is not mutated, so it
/// reappears when the terminal is enlarged again.
pub fn effective_sidebar_visible(app: &App, width: u16) -> bool {
    if !app.sidebar_visible { return false; }
    if width < 88 { return false; }
    let activity = if width >= 24 { 3u16 } else { 2 };
    let requested = app.config.sidebar_width;
    // Keep at least 48 columns for code.
    width.saturating_sub(activity + requested) >= 48
}

/// Minimap needs real estate: hide it on compact screens and short terminals.
pub fn minimap_visible(app: &App, editor_width: u16, editor_height: u16, narrow: bool) -> bool {
    app.config.minimap
        && !narrow
        && editor_width >= 64
        && editor_height >= 10
}

/// Compute all UI regions. The same function drives rendering, mouse
/// hit-testing and PTY sizing, so clicks and shell wrapping always line up.
pub fn compute(app: &App, area: Rect) -> Rects {
    let width = area.width;
    let height = area.height;
    if width == 0 || height == 0 {
        return Rects {
            menu: empty(), activity: empty(), sidebar: empty(), main: empty(), tabs: empty(),
            breadcrumbs: empty(), editor: empty(), gutter: empty(), editor_text: empty(),
            minimap: empty(), panel: empty(), panel_tabs: empty(), panel_body: empty(), status: empty(),
        };
    }

    // Breakpoints used by every region.
    let narrow = width < 100 || height < 16;
    let micro = width < 48 || height < 8;

    let status_h = if app.config.status_bar_visible && height >= 8 && !micro { 1 } else { 0 };
    let menu_h = if height >= 6 && !micro { 1 } else { 0 };
    let status = Rect { x: area.x, y: area.y + height - status_h, width, height: status_h };
    let menu = Rect { x: area.x, y: area.y, width, height: menu_h };
    let body = Rect { x: area.x, y: area.y + menu_h, width, height: height.saturating_sub(status_h + menu_h) };

    let activity_w = if body.width >= 24 { 3u16 } else { 2 };
    let sidebar_w = if effective_sidebar_visible(app, body.width) {
        app.config.sidebar_width
            .min(body.width.saturating_sub(activity_w + 48))
            .max(12)
    } else { 0 };

    let activity = Rect { x: body.x, y: body.y, width: activity_w, height: body.height };
    let sidebar = Rect { x: body.x + activity_w, y: body.y, width: sidebar_w, height: body.height };
    let main = Rect {
        x: body.x + activity_w + sidebar_w,
        y: body.y,
        width: body.width.saturating_sub(activity_w + sidebar_w),
        height: body.height,
    };

    // Reserve at least 5 rows for the editor when the panel is visible.
    let panel_h = if app.panel.visible && main.height >= 9 {
        (app.config.panel_height as u16)
            .min(main.height.saturating_sub(6))
            .max(if main.height >= 12 { 5 } else { 3 })
    } else { 0 };
    let panel = Rect { x: main.x, y: main.y + main.height - panel_h, width: main.width, height: panel_h };
    let top = Rect { x: main.x, y: main.y, width: main.width, height: main.height.saturating_sub(panel_h) };

    let tabs_h = if top.height >= 3 { 1u16 } else { 0 };
    let crumbs_h = if top.height >= 7 && !narrow { 1u16 } else { 0 };
    let tabs = Rect { x: top.x, y: top.y, width: top.width, height: tabs_h };
    let breadcrumbs = Rect { x: top.x, y: top.y + tabs_h, width: top.width, height: crumbs_h };
    let editor = Rect {
        x: top.x,
        y: top.y + tabs_h + crumbs_h,
        width: top.width,
        height: top.height.saturating_sub(tabs_h + crumbs_h),
    };

    let lines = app.active_editor().map(|e| e.buffer.len_lines()).unwrap_or(1);
    let gutter_w = if app.config.show_line_numbers && editor.width >= 36 {
        (lines.to_string().len() as u16 + 3).min(7)
    } else { 0 };
    let minimap_w = if minimap_visible(app, editor.width, editor.height, narrow) { 8u16 } else { 0 };
    let gutter = Rect { x: editor.x, y: editor.y, width: gutter_w, height: editor.height };
    let editor_text = Rect {
        x: editor.x + gutter_w,
        y: editor.y,
        width: editor.width.saturating_sub(gutter_w + minimap_w),
        height: editor.height,
    };
    let minimap = Rect {
        x: editor.x + editor.width.saturating_sub(minimap_w),
        y: editor.y,
        width: minimap_w,
        height: editor.height,
    };

    let panel_tabs_h = if panel_h > 0 { 1u16 } else { 0 };
    let panel_tabs = Rect { x: panel.x, y: panel.y, width: panel.width, height: panel_tabs_h };
    let panel_body = Rect {
        x: panel.x,
        y: panel.y + panel_tabs_h,
        width: panel.width,
        height: panel.height.saturating_sub(panel_tabs_h),
    };

    Rects { menu, activity, sidebar, main, tabs, breadcrumbs, editor, gutter, editor_text, minimap, panel, panel_tabs, panel_body, status }
}

impl Rects {
    pub fn contains(&self, rect: Rect, col: u16, row: u16) -> bool {
        col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::services::Config;
    use crate::themes::Theme;
    use ratatui::layout::Rect;

    fn app_at(w: u16, h: u16) -> App {
        App::new(Config::default(), Theme::default(), None, w as usize, h as usize)
    }

    #[test]
    fn minimap_hidden_on_narrow_terminals() {
        let app = app_at(80, 24);
        let r = compute(&app, Rect { x: 0, y: 0, width: 80, height: 24 });
        assert_eq!(r.minimap.width, 0);
        assert_eq!(r.sidebar.width, 0);
    }

    #[test]
    fn layout_at_120x40_and_160x50_has_no_overlap() {
        for (w, h) in [(120u16, 40u16), (160, 50)] {
            let app = app_at(w, h);
            let r = compute(&app, Rect { x: 0, y: 0, width: w, height: h });
            assert!(r.minimap.width > 0, "minimap should show at {}x{}", w, h);
            assert!(r.sidebar.width > 0);
            assert_eq!(r.activity.x + r.activity.width, r.sidebar.x);
            assert_eq!(r.sidebar.x + r.sidebar.width, r.main.x);
            assert!(r.editor_text.x + r.editor_text.width <= r.minimap.x + r.minimap.width);
            assert!(r.status.y + r.status.height <= h);
        }
    }

    #[test]
    fn panel_never_eats_the_editor() {
        let mut app = app_at(80, 24);
        app.panel.visible = true;
        app.config.panel_height = 20;
        let r = compute(&app, Rect { x: 0, y: 0, width: 80, height: 24 });
        assert!(r.editor.height >= 4, "editor must keep at least 4 rows");
        assert!(r.panel_body.height >= 2);
    }
}
