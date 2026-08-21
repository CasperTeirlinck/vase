//! The palette as AppKit sees it: each accessor resolves a core `Role` into an `NSColor`.
//!
//! Under the native style the colors are the system's own, so the chrome follows the user's
//! appearance and accent color instead of a configured palette.

use objc2::rc::Retained;
use objc2_app_kit::{NSBezierPath, NSColor};
use objc2_foundation::NSPoint;
use vase_core::chrome::theme::{palette, style, vase_mark, Role, Style};
use vase_core::geometry::Rect;

pub(crate) use vase_core::chrome::theme::{PANE_PAD, PANE_RADIUS};

/// Corner radius of a native card, matching an AppKit popover's.
const CARD_RADIUS: f64 = 12.0;

/// Corner radius of a card or a pane placeholder in the current style.
pub(crate) fn card_radius() -> f64 {
    match style() {
        Style::Native => CARD_RADIUS,
        Style::Powerline => PANE_RADIUS,
    }
}

/// Resolve a palette role to an `NSColor`, for laid-out chrome that names colors by role.
pub(crate) fn role(r: Role) -> Retained<NSColor> {
    color(r)
}

fn color(role: Role) -> Retained<NSColor> {
    match style() {
        Style::Native => system(role),
        Style::Powerline => configured(role),
    }
}

fn configured(role: Role) -> Retained<NSColor> {
    let c = palette().color(role);
    NSColor::colorWithSRGBRed_green_blue_alpha(c[0], c[1], c[2], c[3])
}

/// The system's own color per role. `Bg` is clear: under the native style the glass backdrop is the background.
fn system(role: Role) -> Retained<NSColor> {
    match role {
        Role::Bg => NSColor::clearColor(),
        Role::Active => NSColor::selectedContentBackgroundColor(),
        Role::DimBg => NSColor::unemphasizedSelectedContentBackgroundColor(),
        Role::Text => NSColor::labelColor(),
        Role::Dim => NSColor::secondaryLabelColor(),
        Role::Accent | Role::Hotkey => NSColor::controlAccentColor(),
        Role::Badge => NSColor::systemRedColor(),
        Role::Border => NSColor::separatorColor(),
    }
}

pub(crate) fn strip_bg() -> Retained<NSColor> {
    color(Role::Bg)
}
pub(crate) fn active_bg() -> Retained<NSColor> {
    color(Role::Active)
}
pub(crate) fn text_col() -> Retained<NSColor> {
    color(Role::Text)
}
pub(crate) fn dim_col() -> Retained<NSColor> {
    color(Role::Dim)
}
// The mark and every highlight (focus outline, current-row accent, off-workspace and favorite markers, armed-prefix dot).
pub(crate) fn accent() -> Retained<NSColor> {
    color(Role::Accent)
}
pub(crate) fn badge_red() -> Retained<NSColor> {
    color(Role::Badge)
}
pub(crate) fn tab_border() -> Retained<NSColor> {
    color(Role::Border)
}
pub(crate) fn hotkey_border() -> Retained<NSColor> {
    color(Role::Hotkey)
}
// The focused-pane accent (outline / placeholder border).
pub(crate) fn pane_border() -> Retained<NSColor> {
    accent()
}

/// Text drawn on top of the `Active` fill, whose emphasis the primary text color cannot survive.
pub(crate) fn active_text(r: Role) -> Retained<NSColor> {
    match style() {
        // The system pairs its selection fill with this text color; secondary runs keep their step down in emphasis.
        Style::Native => match r {
            Role::Dim => NSColor::alternateSelectedControlTextColor().colorWithAlphaComponent(0.7),
            _ => NSColor::alternateSelectedControlTextColor(),
        },
        Style::Powerline => color(r),
    }
}

/// The vase brand mark as a bezier path filling the box `[x, x+w] × [y, y+h]`; normalized `y` is flipped so the vase stands up (bottom-left origin).
pub(crate) fn vase_mark_bezier(x: f64, y: f64, w: f64, h: f64) -> Retained<NSBezierPath> {
    let path = NSBezierPath::new();
    for (i, (px, py)) in vase_mark().polygon(Rect::new(x, y, w, h)).into_iter().enumerate() {
        let p = NSPoint::new(px, py);
        if i == 0 {
            path.moveToPoint(p);
        } else {
            path.lineToPoint(p);
        }
    }
    path.closePath();
    path
}
