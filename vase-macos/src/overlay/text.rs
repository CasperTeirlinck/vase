//! Text/icon helpers shared by the tab bar and the switcher.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::AnyThread;
use objc2_app_kit::NSColor;
#[allow(deprecated)]
use objc2_app_kit::NSObliquenessAttributeName;
use objc2_app_kit::{NSAttributedStringAttachmentConveniences, NSAttributedStringNSStringDrawing, NSFont, NSFontAttributeName, NSForegroundColorAttributeName, NSImage, NSTextAttachment, NSWorkspace};
use objc2_foundation::{NSAttributedString, NSDictionary, NSNumber, NSPoint, NSRect, NSSize, NSString};
use std::cell::RefCell;
use std::collections::HashMap;
use vase_core::chrome::theme::{style, Style};

use super::theme::text_col;

/// The font the chrome draws every label in: the system font under the native style, a monospaced
/// one under the powerline style, whose fixed pitch keeps the interlocking tabs' widths predictable.
pub(crate) fn chrome_font(size: f64) -> Retained<NSFont> {
    match style() {
        Style::Native => NSFont::systemFontOfSize(size),
        Style::Powerline => NSFont::monospacedSystemFontOfSize_weight(size, 0.0),
    }
}

/// Width of `text` at `size` points, in the font the chrome draws it in.
pub(crate) fn measure(text: &str, size: f64) -> f64 {
    segment(text, &chrome_font(size), &text_col(), None).size().width
}

thread_local! {
    static ICON_CACHE: RefCell<HashMap<String, Option<Retained<NSImage>>>> =
        RefCell::new(HashMap::new());
}

/// Resolve and cache `name`'s app icon (misses too). Blocking; pre-warm off the render hot path.
#[allow(deprecated)]
pub fn prewarm_icon(name: &str) {
    ICON_CACHE.with(|c| {
        if c.borrow().contains_key(name) {
            return;
        }
        let ws = NSWorkspace::sharedWorkspace();
        // A window's owner name is the process name, which can differ from the `.app` file name (e.g. "Code" vs
        // "Visual Studio Code.app"), so match a running app by localized name first; fall back to a name->path
        // lookup for apps that aren't running (the launcher list, whose names are `.app` file stems).
        let icon = ws
            .runningApplications()
            .iter()
            .find(|a| a.localizedName().is_some_and(|n| n.to_string() == name))
            .and_then(|a| a.icon())
            .or_else(|| ws.fullPathForApplication(&NSString::from_str(name)).map(|path| ws.iconForFile(&path)));
        c.borrow_mut().insert(name.to_string(), icon);
    });
}

/// The already-cached icon for `name`, or `None` if not yet warmed (or a miss). Never blocks, so the render hot path can't stall on a lookup.
pub(crate) fn app_icon(name: &str) -> Option<Retained<NSImage>> {
    ICON_CACHE.with(|c| c.borrow().get(name).cloned().flatten())
}

/// An attributed string wrapping the app icon as a square, line-height attachment.
pub(crate) fn icon_run(name: &str, size: f64, font: &NSFont) -> Option<Retained<NSAttributedString>> {
    let img = app_icon(name)?;
    let att = NSTextAttachment::new();
    att.setImage(Some(&img));
    // The attachment's y-origin is its offset from the text baseline, so (capHeight − size)/2 centers the icon's midline on the cap-height midline.
    let y = (font.capHeight() - size) / 2.0;
    att.setBounds(NSRect::new(NSPoint::new(0.0, y), NSSize::new(size, size)));
    Some(NSAttributedString::attributedStringWithAttachment(&att))
}

/// One styled run: font + color, optionally an obliqueness (fake-italic slant).
// NSObliquenessAttributeName is deprecated under TextKit 2 but is still the only fake-italic knob; NSTextField honors it via its TextKit 1 fallback.
#[allow(deprecated)]
pub(crate) fn segment(text: &str, font: &NSFont, color: &NSColor, obliqueness: Option<&NSNumber>) -> Retained<NSAttributedString> {
    let s = NSString::from_str(text);
    let font_any: &AnyObject = font.as_ref();
    let color_any: &AnyObject = color.as_ref();
    let dict = match obliqueness {
        Some(n) => {
            let n_any: &AnyObject = n.as_ref();
            NSDictionary::from_slices(&[unsafe { NSFontAttributeName }, unsafe { NSForegroundColorAttributeName }, unsafe { NSObliquenessAttributeName }], &[font_any, color_any, n_any])
        }
        None => NSDictionary::from_slices(&[unsafe { NSFontAttributeName }, unsafe { NSForegroundColorAttributeName }], &[font_any, color_any]),
    };
    // SAFETY: `dict` holds the correct attribute-key/value types.
    unsafe { NSAttributedString::initWithString_attributes(NSAttributedString::alloc(), &s, Some(&dict)) }
}
