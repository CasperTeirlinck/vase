//! Text/icon helpers shared by the tab bar and the switcher.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::AnyThread;
use objc2_app_kit::{
    NSAttributedStringAttachmentConveniences, NSFont, NSFontAttributeName,
    NSForegroundColorAttributeName, NSImage, NSTextAttachment, NSWorkspace,
};
#[allow(deprecated)]
use objc2_app_kit::NSObliquenessAttributeName;
use objc2_app_kit::NSColor;
use objc2_foundation::{
    NSAttributedString, NSDictionary, NSNumber, NSPoint, NSRect, NSSize, NSString,
};
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static ICON_CACHE: RefCell<HashMap<String, Option<Retained<NSImage>>>> =
        RefCell::new(HashMap::new());
}

/// Resolve `name`'s icon via NSWorkspace and cache it (incl. misses). Blocking
/// (a Launch Services lookup + icon load) — call off the hot path to pre-warm.
/// fullPathForApplication is deprecated in favor of the bundle-id/URL APIs, but it's
/// the only one that maps a plain app *name* to a path — which is all we have here.
#[allow(deprecated)]
pub fn prewarm_icon(name: &str) {
    ICON_CACHE.with(|c| {
        if c.borrow().contains_key(name) {
            return;
        }
        let ws = NSWorkspace::sharedWorkspace();
        let icon = ws
            .fullPathForApplication(&NSString::from_str(name))
            .map(|path| ws.iconForFile(&path));
        c.borrow_mut().insert(name.to_string(), icon);
    });
}

/// The already-cached icon for `name`, or `None` if not yet warmed (or a miss).
/// Never blocks — the render hot path uses this so it can't stall on a lookup.
pub(crate) fn app_icon(name: &str) -> Option<Retained<NSImage>> {
    ICON_CACHE.with(|c| c.borrow().get(name).cloned().flatten())
}

/// An attributed string wrapping the app icon as a square, line-height attachment.
pub(crate) fn icon_run(name: &str, size: f64, font: &NSFont) -> Option<Retained<NSAttributedString>> {
    let img = app_icon(name)?;
    let att = NSTextAttachment::new();
    att.setImage(Some(&img));
    // The attachment's y-origin is its offset from the text baseline, so
    // (capHeight − size)/2 centers the icon's midline on the cap-height midline.
    let y = (font.capHeight() - size) / 2.0;
    att.setBounds(NSRect::new(NSPoint::new(0.0, y), NSSize::new(size, size)));
    Some(NSAttributedString::attributedStringWithAttachment(&att))
}

/// Scroll offset so a `selected` row stays within a window of `visible` rows.
pub(crate) fn scroll_offset(selected: usize, visible: usize) -> usize {
    if visible == 0 || selected < visible {
        0
    } else {
        selected - visible + 1
    }
}

/// One styled run: font + color, optionally an obliqueness (fake-italic slant).
// NSObliquenessAttributeName is deprecated under TextKit 2 but is still the
// only fake-italic knob; NSTextField honors it via its TextKit 1 fallback.
#[allow(deprecated)]
pub(crate) fn segment(
    text: &str,
    font: &NSFont,
    color: &NSColor,
    obliqueness: Option<&NSNumber>,
) -> Retained<NSAttributedString> {
    let s = NSString::from_str(text);
    let font_any: &AnyObject = font.as_ref();
    let color_any: &AnyObject = color.as_ref();
    let dict = match obliqueness {
        Some(n) => {
            let n_any: &AnyObject = n.as_ref();
            NSDictionary::from_slices(
                &[
                    unsafe { NSFontAttributeName },
                    unsafe { NSForegroundColorAttributeName },
                    unsafe { NSObliquenessAttributeName },
                ],
                &[font_any, color_any, n_any],
            )
        }
        None => NSDictionary::from_slices(
            &[unsafe { NSFontAttributeName }, unsafe { NSForegroundColorAttributeName }],
            &[font_any, color_any],
        ),
    };
    // SAFETY: `dict` holds the correct attribute-key/value types.
    unsafe { NSAttributedString::initWithString_attributes(NSAttributedString::alloc(), &s, Some(&dict)) }
}

#[cfg(test)]
#[path = "text_test.rs"]
mod tests;
