//! Display geometry: primary-screen coordinate anchoring and per-display bounds.

use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;
use objc2_foundation::{NSNumber, NSRect, NSString};
use vase_core::backend::Display;
use vase_core::geometry::Rect;

/// The primary display: `screens()[0]`, the menu-bar display and anchor of the AppKit/CG coordinate origin.
/// NOT `mainScreen`, which follows keyboard focus and on multi-monitor can be a secondary display,
/// breaking every coordinate flip that assumes the primary's origin.
pub(crate) fn primary_screen(mtm: MainThreadMarker) -> Option<Retained<NSScreen>> {
    NSScreen::screens(mtm).iter().next()
}

pub(crate) fn primary_screen_height(mtm: MainThreadMarker) -> Option<f64> {
    Some(primary_screen(mtm)?.frame().size.height)
}

/// The stable CoreGraphics display id (`NSScreenNumber`) of an NSScreen, so tabs can be matched
/// to the same physical monitor across a hotplug/reorder.
fn screen_number(screen: &NSScreen) -> Option<u32> {
    let key = NSString::from_str("NSScreenNumber");
    let value = screen.deviceDescription().objectForKey(&key)?;
    value.downcast::<NSNumber>().ok().map(|n| n.unsignedIntValue())
}

/// Every display in CG top-left coords, ordered left-to-right then top-to-bottom so a screen's index
/// matches its physical layout. `work_area` excludes that display's menu bar and Dock.
pub fn all_screens(mtm: MainThreadMarker) -> Vec<Display> {
    let Some(primary) = primary_screen(mtm) else {
        return Vec::new();
    };
    let ph = primary.frame().size.height;
    let to_cg = |f: NSRect| Rect::new(f.origin.x, ph - (f.origin.y + f.size.height), f.size.width, f.size.height);
    let mut displays: Vec<Display> = NSScreen::screens(mtm).iter().map(|s| Display { id: screen_number(&s).unwrap_or(0), bounds: to_cg(s.frame()), work_area: to_cg(s.visibleFrame()) }).collect();
    displays.sort_by(|a, b| a.bounds.x.total_cmp(&b.bounds.x).then(a.bounds.y.total_cmp(&b.bounds.y)));
    displays
}
