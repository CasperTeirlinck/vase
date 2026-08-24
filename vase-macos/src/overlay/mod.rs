//! Non-activating, always-on-top overlay windows (AppKit). Main-thread only.

mod glass;
mod help;
mod painter;
mod panel;
mod panes;
mod screens;
mod switcher;
mod tab_bar;
pub(crate) mod text;
mod theme;

pub use painter::AppKitPainter;
pub use screens::all_screens;
pub(crate) use theme::vase_mark_bezier;

pub use vase_core::chrome::{bar_height, FAVORITE_MARK, FONT_SIZE, WORKSPACE_MARK};

use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

/// Initialize NSApp as an accessory (no Dock icon) so AppKit windows render.
pub fn nsapp_init(mtm: MainThreadMarker) {
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    app.finishLaunching();
}
