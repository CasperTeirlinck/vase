//! Display geometry: primary-screen coordinate anchoring and per-display bounds.

use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;
use objc2_foundation::{NSNumber, NSRect, NSString};
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

/// Each display's `(display id, full bounds, usable/visible area)` in CG top-left coords, in NSScreen order.
/// The visible area excludes that display's menu bar and Dock, so it's the correct area to tile and place overlays.
pub fn all_screens(mtm: MainThreadMarker) -> Vec<(u32, Rect, Rect)> {
    let Some(primary) = primary_screen(mtm) else {
        return Vec::new();
    };
    let ph = primary.frame().size.height;
    let to_cg = |f: NSRect| Rect::new(f.origin.x, ph - (f.origin.y + f.size.height), f.size.width, f.size.height);
    NSScreen::screens(mtm).iter().map(|s| (screen_number(&s).unwrap_or(0), to_cg(s.frame()), to_cg(s.visibleFrame()))).collect()
}

/// Bounding box of the given CG rects, used to size an overlay panel to just its content (a window can't span displays under "separate Spaces").
pub(crate) fn bbox(rects: &[Rect]) -> Rect {
    let Some(first) = rects.first() else {
        return Rect::new(0.0, 0.0, 0.0, 0.0);
    };
    let (mut x0, mut y0) = (first.x, first.y);
    let (mut x1, mut y1) = (first.x + first.w, first.y + first.h);
    for r in &rects[1..] {
        x0 = x0.min(r.x);
        y0 = y0.min(r.y);
        x1 = x1.max(r.x + r.w);
        y1 = y1.max(r.y + r.h);
    }
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}
