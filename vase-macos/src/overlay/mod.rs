//! A non-activating, always-on-top tab-bar overlay window (AppKit).
//! vase's own window — never touches user windows. Main-thread only.

mod panes;
mod screens;
mod switcher;
mod tab_bar;
mod text;
mod theme;

pub use panes::{FocusBorder, PaneOverlay};
pub use screens::all_screens;
pub use switcher::{SwitchRow, SwitcherView};
pub use tab_bar::{BarTab, TabBar};
pub(crate) use tab_bar::vase_mark_bezier;
pub use text::prewarm_icon;

use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

/// Height of the tab bar. The daemon reserves this much at the bottom of a
/// stack's region so the bar sits below the windows instead of over them.
pub const BAR_HEIGHT: f64 = 22.0;

pub(crate) const FONT_SIZE: f64 = 12.0;

/// Initialize NSApp as an accessory (no Dock icon) so AppKit windows render.
pub fn nsapp_init(mtm: MainThreadMarker) {
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    app.finishLaunching();
}
