//! Chrome is everything vase paints on top of the windows.

pub mod bar;
pub(crate) mod deck;
mod paint;
pub mod theme;

pub use crate::geometry::{Rect, BAR_HEIGHT};
pub use deck::{Context, Deck};
pub use paint::{ListAt, Painter, SwitchRow};

/// The tileable area of a display: its work area, less the strip the tab bar reserves on the main one.
pub fn usable(work_area: Rect, main: bool) -> Rect {
    if main {
        Rect::new(work_area.x, work_area.y, work_area.w, work_area.h - BAR_HEIGHT)
    } else {
        work_area
    }
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
