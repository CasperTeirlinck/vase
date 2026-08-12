//! Chrome is everything vase paints on top of the windows.

pub mod bar;
pub mod theme;

pub use crate::geometry::BAR_HEIGHT;

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
