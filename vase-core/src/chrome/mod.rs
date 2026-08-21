//! Chrome is everything vase paints on top of the windows.

pub mod bar;
pub(crate) mod deck;
mod paint;
pub mod powerline;
pub mod theme;

pub use crate::geometry::Rect;
pub use deck::{Context, Deck};
pub use paint::{ListAt, Painter, SwitchRow};

/// Which edge of the main display the tab bar sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Top,
    Bottom,
}

/// Height of a bar strip, which the theme's style decides.
pub fn bar_height() -> f64 {
    theme::style().bar_height()
}

/// The tileable area of a display: its work area, less the strip the tab bar reserves on the main one.
pub fn usable(work_area: Rect, main: bool, bar: Position) -> Rect {
    if !main {
        return work_area;
    }
    let strip = bar_height();
    let top = match bar {
        Position::Top => work_area.y + strip,
        Position::Bottom => work_area.y,
    };
    Rect::new(work_area.x, top, work_area.w, work_area.h - strip)
}

pub const FONT_SIZE: f64 = 12.0;

/// Leading marker on a tab or row whose window is on another OS workspace.
pub const WORKSPACE_MARK: &str = "◇";

/// Leading marker on a favorite app row in the picker.
pub const FAVORITE_MARK: &str = "★";

/// Scroll offset so a `selected` row stays within a window of `visible` rows.
pub fn scroll_offset(selected: usize, visible: usize) -> usize {
    if visible == 0 || selected < visible {
        0
    } else {
        selected - visible + 1
    }
}
