//! Non-activating, always-on-top overlay windows (AppKit). Main-thread only.

mod deck;
mod panel;
mod panes;
mod screens;
mod switcher;
mod tab_bar;
pub(crate) mod text;
mod theme;

pub(crate) use theme::{by_name, parse_hex, set_mark, set_theme, Mark, Theme, ONE_DARK};

pub use deck::{Chrome, Overlays};
pub use panes::{FocusBorder, PaneOverlay};
pub use screens::all_screens;
pub use switcher::{SwitchRow, SwitcherView};
pub(crate) use tab_bar::vase_mark_bezier;
pub use tab_bar::{BarTab, TabBar};
pub use text::prewarm_icon;

use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

/// Height of the tab bar.
pub const BAR_HEIGHT: f64 = 22.0;

pub(crate) const FONT_SIZE: f64 = 12.0;

/// Leading marker on a tab or row whose window is on another (non-current) macOS Space.
pub(crate) const SPACE_MARK: &str = "◇";

/// Leading marker on a favorite app row in the picker.
pub(crate) const FAVORITE_MARK: &str = "★";

/// Initialize NSApp as an accessory (no Dock icon) so AppKit windows render.
pub fn nsapp_init(mtm: MainThreadMarker) {
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    app.finishLaunching();
}
