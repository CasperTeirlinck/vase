//! Liquid Glass: the material the native chrome is drawn on.

use std::sync::OnceLock;

use objc2::rc::Retained;
use objc2::runtime::AnyClass;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSGlassEffectView, NSGlassEffectViewStyle, NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView};
use objc2_foundation::NSRect;

/// A glass panel filling `frame` with `radius`-rounded corners, holding `content`.
///
/// Content goes *inside* the material rather than on top of it: that is what earns it the system's
/// own legibility treatment, and the glass makes no promises about views it does not own.
///
/// Liquid Glass arrived in macOS 26; before that the vibrancy material it grew out of is the closest thing the system has.
pub(crate) fn backdrop(mtm: MainThreadMarker, frame: NSRect, radius: f64, content: Option<&NSView>) -> Retained<NSView> {
    if liquid_glass() {
        let glass = NSGlassEffectView::initWithFrame(NSGlassEffectView::alloc(mtm), frame);
        glass.setStyle(NSGlassEffectViewStyle::Regular);
        glass.setCornerRadius(radius);
        glass.setContentView(content);
        return Retained::into_super(glass);
    }
    let vibrancy = NSVisualEffectView::initWithFrame(NSVisualEffectView::alloc(mtm), frame);
    vibrancy.setMaterial(NSVisualEffectMaterial::HUDWindow);
    // Blur what is behind the panel: the user's windows, not the panel's own (empty) background.
    vibrancy.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    // The chrome's panel is never key, and vibrancy that followed the window's active state would grey out.
    vibrancy.setState(NSVisualEffectState::Active);
    if let Some(layer) = vibrancy.layer() {
        layer.setCornerRadius(radius);
        layer.setMasksToBounds(true);
    }
    if let Some(content) = content {
        vibrancy.addSubview(content);
    }
    Retained::into_super(vibrancy)
}

/// Whether this macOS draws Liquid Glass, which the class only exists from macOS 26 on.
fn liquid_glass() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| AnyClass::get(c"NSGlassEffectView").is_some())
}
