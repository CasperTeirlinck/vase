//! Non-activating, always-on-top overlay windows (AppKit). Main-thread only.

mod deck;
mod panel;
mod panes;
mod screens;
mod switcher;
mod tab_bar;
pub(crate) mod text;
mod theme;

pub use deck::{Chrome, Overlays};
pub use panes::{FocusBorder, PaneOverlay};
pub use screens::all_screens;
pub use switcher::{SwitchRow, SwitcherView};
pub(crate) use tab_bar::vase_mark_bezier;
pub use tab_bar::TabBar;
pub use text::prewarm_icon;

pub use vase_core::chrome::bar::BarTab;
pub use vase_core::chrome::{BAR_HEIGHT, FAVORITE_MARK, FONT_SIZE, WORKSPACE_MARK};

use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use vase_core::geometry::Rect;

/// The tileable area of a display: its work area, less the strip the tab bar reserves on the main one.
pub fn usable(work_area: Rect, main: bool) -> Rect {
    if main {
        Rect::new(work_area.x, work_area.y, work_area.w, work_area.h - BAR_HEIGHT)
    } else {
        work_area
    }
}

/// Initialize NSApp as an accessory (no Dock icon) so AppKit windows render.
pub fn nsapp_init(mtm: MainThreadMarker) {
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    app.finishLaunching();
}
